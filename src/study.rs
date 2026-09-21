//! Native experiment sessions; no Unity process or local server is required.
use crate::{
    audio, auditory,
    experiment_set::{self, Manifest, Trial},
    App,
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs::File,
    io::Write,
    sync::{Mutex, MutexGuard},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::Manager;

#[derive(Default)]
pub struct Runtime(pub Mutex<Option<Session>>);
pub struct Session {
    manifest: Manifest,
    trials: Vec<Trial>,
    index: usize,
    log: File,
    path: String,
    id: String,
    calibrated: bool,
    checked: u8,
    calibration: Option<(usize, usize, f64)>,
    active: Option<Active>,
    fault: Option<String>,
}
struct Active {
    generation: usize,
    onset_ms: f64,
    duration_ms: f64,
    anchor: Option<Instant>,
    response: Option<Value>,
}
fn unix_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
        * 1000.
}
impl Runtime {
    // Hold the guard through competing edits/preview starts, closing the start-session race.
    pub fn idle(&self) -> Result<MutexGuard<'_, Option<Session>>, String> {
        let locked = self.0.lock().map_err(|e| e.to_string())?;
        if locked.is_some() {
            return Err("先にAirCueの実験セッションを終了してください".into());
        }
        Ok(locked)
    }
    pub fn tick(&self, audio: &audio::Audio) {
        if let Ok(mut lock) = self.0.lock() {
            if let Some(s) = lock.as_mut() {
                if s.fault.is_some() {
                    return;
                }
                if let Err(e) = s.observe(&audio.status(), Instant::now()) {
                    s.fault = Some(e);
                    s.calibrated = false;
                    audio.stop();
                }
            }
        }
    }
    pub fn status(&self) -> Value {
        let lock = self.0.lock().unwrap();
        match lock.as_ref() {
            None => json!({"open":false}),
            Some(s) => {
                json!({"open":true,"participant":s.trials[0].participant_id,"completed":s.index,"total":s.trials.len(),"block":s.trials.get(s.index).map(|t|t.block),"calibrated":s.calibrated,"checked":s.checked,"active":s.active.is_some(),"responded":s.active.as_ref().is_some_and(|a|a.response.is_some()),"primary":s.manifest.plan.primary,"secondary":s.manifest.plan.secondary,"logPath":s.path,"fault":s.fault})
            }
        }
    }
    pub fn end(&self, audio: &audio::Audio, reason: &str) -> Result<(), String> {
        let mut lock = self.0.lock().unwrap();
        audio.stop();
        if let Some(mut s) = lock.take() {
            if s.active.is_some() {
                s.record("aborted", json!({"reason":reason,"primaryResponse":s.active.as_ref().and_then(|a|a.response.clone())}))?;
            }
            s.record(
                "sessionEnded",
                json!({"reason":reason,"completed":s.index,"total":s.trials.len()}),
            )?;
        }
        Ok(())
    }
}
impl Session {
    fn record(&mut self, event: &str, detail: Value) -> Result<(), String> {
        let row = json!({"schemaVersion":1,"runner":"AirCue","sessionId":self.id,"participantId":self.trials[0].participant_id,"projectHash":self.manifest.project_hash,"trial":if self.active.is_some(){self.trials.get(self.index)}else{None},"event":event,"unixMs":unix_ms(),"detail":detail});
        let result = (|| {
            let mut bytes = serde_json::to_vec(&row).map_err(|e| e.to_string())?;
            bytes.push(b'\n');
            self.log.write_all(&bytes).map_err(|e| e.to_string())?;
            self.log.sync_data().map_err(|e| e.to_string())
        })();
        if let Err(e) = &result {
            self.fault = Some(format!("ログを保存できません: {e}"));
            self.calibrated = false;
        }
        result
    }
    fn ready(&self) -> Result<(), String> {
        if let Some(e) = &self.fault {
            Err(e.clone())
        } else {
            Ok(())
        }
    }
    fn observe(&mut self, status: &audio::Status, now: Instant) -> Result<(), String> {
        self.ready()?;
        if let Some((index, generation, duration)) = self.calibration {
            if status.generation != generation || status.error.is_some() {
                self.calibration = None;
                self.calibrated = false;
                return Err(
                    "確認音の再生が中断されました。セッションを終了して出力を確認してください"
                        .into(),
                );
            }
            if !status.playing {
                self.calibration = None;
                if status.position_ms + 1. >= duration {
                    self.checked |= 1 << index;
                    self.record(
                        "calibrationPlayed",
                        json!({"channel":index,"output":status.output}),
                    )?;
                }
            }
        }
        if let Some(a) = self.active.as_mut() {
            if status.generation != a.generation
                || status.error.is_some()
                || (!status.playing && status.position_ms + 1. < a.duration_ms)
            {
                self.record(
                    "aborted",
                    json!({"reason":"playbackInterrupted","audioError":status.error}),
                )?;
                self.active = None;
                self.index += 1;
                self.calibrated = false;
                self.checked = 0;
                return Err(
                    "試行の再生が中断されました。セッションを終了して出力を確認してください".into(),
                );
            }
            if a.anchor.is_none() && status.position_ms >= a.onset_ms {
                // Frame progress is a software estimate, not a hardware presentation timestamp.
                let elapsed = (status.position_ms - a.onset_ms).min(a.duration_ms).max(0.);
                a.anchor = now.checked_sub(Duration::from_secs_f64(elapsed / 1000.));
                self.record("startedObserved",json!({"observedUnixMs":unix_ms(),"estimatedOnsetUnixMs":unix_ms()-elapsed,"positionMs":status.position_ms,"clock":"audio-frame-progress/monotonic-estimate","output":status.output}))?;
            }
        }
        Ok(())
    }
    fn respond(&mut self, answer: &str, now: Instant) -> Result<(), String> {
        self.ready()?;
        let a = self.active.as_ref().ok_or("再生中の試行がありません")?;
        if a.response.is_some() {
            return Err("回答は記録済みです".into());
        }
        let anchor = a.anchor.ok_or("刺激開始前には回答できません")?;
        let allowed = if self.manifest.plan.primary == "direction" {
            &["left", "right", "front", "back", "uncertain"][..]
        } else {
            &["detected"][..]
        };
        if !allowed.contains(&answer) {
            return Err("回答が不正です".into());
        }
        let response = json!({"answer":answer,"reactionTimeMs":now.saturating_duration_since(anchor).as_secs_f64()*1000.,"clock":"monotonic; backend receipt; estimated software onset"});
        self.record("response", response.clone())?;
        self.active.as_mut().unwrap().response = Some(response);
        Ok(())
    }
    fn finish(&mut self, ratings: BTreeMap<String, f64>) -> Result<(), String> {
        self.ready()?;
        let a = self.active.as_ref().ok_or("試行がありません")?;
        let response = a.response.clone().ok_or("先に主指標へ回答してください")?;
        if ratings.len() != self.manifest.plan.secondary.len()
            || self.manifest.plan.secondary.iter().any(|k| {
                !ratings
                    .get(k)
                    .is_some_and(|v| v.is_finite() && (0.0..=10.0).contains(v))
            })
        {
            return Err("選択した補助指標すべてに0〜10で回答してください".into());
        }
        self.record(
            "trialCompleted",
            json!({"response":response,"ratings":ratings}),
        )?;
        self.active = None;
        self.index += 1;
        Ok(())
    }
}

#[tauri::command]
pub async fn start_study(
    app: tauri::AppHandle,
    manifest: Manifest,
    participant: String,
) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let s=app.state::<App>();
        let mut guard=s.study.idle()?;
        if s.experiment.is_running() {return Err("ローカル連携を停止してから開始してください".into());}
        if s.audio.status().playing {return Err("プレビューを停止してから開始してください".into());}
        let p=s.project.lock().unwrap().clone();
        experiment_set::validate_current(&manifest,&p,&s.dir)?;
        let device=p.settings.device_id.as_ref().ok_or("出力設定で4ch機器を選択してください")?;
        let info=audio::devices()?.into_iter().find(|d|&d.id==device && d.usable && d.channels>=4).ok_or("実験には利用可能な4ch出力が必要です")?;
        let trials:Vec<_>=manifest.trials.iter().filter(|t|t.participant_id==participant).cloned().collect();
        if trials.is_empty(){return Err("manifestの参加者を指定してください".into());}
        // Preflight all conditions before opening the session; don't defer clipping errors to a participant.
        for c in &manifest.plan.conditions { auditory::render_project(&experiment_set::stimulus(&p,c)?,&s.dir,"all")?; }
        let id=format!("{}-{}",participant,SystemTime::now().duration_since(UNIX_EPOCH).map_err(|e|e.to_string())?.as_nanos());
        let dir=s.dir.join("study-results"); std::fs::create_dir_all(&dir).map_err(|e|e.to_string())?;
        let path=dir.join(format!("{id}.jsonl"));
        let log=File::options().write(true).create_new(true).open(&path).map_err(|e|e.to_string())?;
        let mut session=Session {manifest,trials,index:0,log,path:path.display().to_string(),id,calibrated:false,checked:0,calibration:None,active:None,fault:None};
        session.record("sessionStarted",json!({"manifest":session.manifest,"runtimeDevice":info,"timing":"software frame estimate; not physical onset"}))?;
        let monitor_id=session.id.clone();
        *guard=Some(session);
        let monitor_app=app.clone();
        if let Err(e)=std::thread::Builder::new().name("aircue-study-observer".into()).spawn(move || {
            loop {
                let state=monitor_app.state::<App>();
                if !state.study.0.lock().unwrap().as_ref().is_some_and(|s|s.id==monitor_id) {break;}
                state.study.tick(&state.audio);
                std::thread::sleep(Duration::from_millis(5));
            }
        }) { *guard=None; return Err(e.to_string()); }
        drop(guard);
        Ok(s.study.status())
    }).await.map_err(|e|e.to_string())?
}
#[tauri::command]
pub fn study_status(s: tauri::State<App>) -> Value {
    s.study.tick(&s.audio);
    s.study.status()
}
#[tauri::command]
pub async fn study_action(
    app: tauri::AppHandle,
    action: String,
    value: Value,
) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let s=app.state::<App>();
        if action=="end" {s.study.end(&s.audio,"operatorEnded")?;return Ok(s.study.status());}
        s.study.tick(&s.audio);
        let mut guard=s.study.0.lock().unwrap();
        let session=guard.as_mut().ok_or("参加者を準備してください")?;
        session.ready()?;
        let result=(|| {
            match action.as_str() {
                "calibration" => {
                    if session.active.is_some() || s.audio.status().playing {return Err("先に再生・回答を完了してください".into());}
                    let index=value.as_u64().filter(|i|*i<4).ok_or("確認する左右を指定してください")? as usize;
                    let (cue,samples)=experiment_set::calibration(&session.manifest.project,&s.dir,index)?;
                    session.calibrated=false;
                    session.checked &= !(1 << index);
                    session.record("calibrationRequested",json!({"channel":index}))?;
                    s.audio.play_extended(cue.settings.device_id.clone().unwrap(),samples,1.,false,[cue.settings.routing[0]-1,cue.settings.routing[1]-1],"study",false,true)?;
                    session.calibration=Some((index,s.audio.status().generation,cue.duration_ms));
                },
                "confirm" => {
                    if session.checked!=15 || session.calibration.is_some() || value!=json!([true,true,true]) {return Err("4種類の確認音を再生し、左右と快適レベルを確認してください".into());}
                    session.record("calibrationConfirmed",json!({"headphones":true,"air":true,"comfortable":true}))?;
                    session.calibrated=true;
                },
                "next" => {
                    if !session.calibrated || session.active.is_some() || s.audio.status().playing {return Err("左右確認または前の試行を完了してください".into());}
                    let trial=session.trials.get(session.index).ok_or("全試行が終了しました")?.clone();
                    experiment_set::validate_current(&session.manifest,&s.project.lock().unwrap(),&s.dir)?;
                    let c=session.manifest.plan.conditions.iter().find(|c|c.id==trial.condition_id).unwrap();
                    let p=experiment_set::stimulus(&session.manifest.project,c)?;
                    let samples=auditory::render_project(&p,&s.dir,"all")?;
                    let onset=p.audio_clips.iter().map(|c|c.start_ms).chain(p.clips.iter().map(|c|c.start_ms)).fold(f64::INFINITY,f64::min);
                    session.active=Some(Active{generation:usize::MAX,onset_ms:onset,duration_ms:p.duration_ms,anchor:None,response:None});
                    session.record("trialRequested",json!({"plannedOnsetOffsetMs":onset,"stimulus":session.manifest.stimuli.iter().find(|v|v["stimulusId"]==trial.condition_id)}))?;
                    if let Err(e)=s.audio.play_extended(p.settings.device_id.unwrap(),samples,1.,false,[p.settings.routing[0]-1,p.settings.routing[1]-1],"study",false,true) {
                        session.record("aborted",json!({"reason":"playbackFailed","error":e}))?;
                        session.active=None;session.index+=1;session.calibrated=false;session.checked=0;return Err(e);
                    }
                    session.active.as_mut().unwrap().generation=s.audio.status().generation;
                },
                "response" => session.respond(value.as_str().ok_or("回答を指定してください")?,Instant::now())?,
                "finish" => {
                    if s.audio.status().playing {return Err("再生終了後に試行を保存してください".into());}
                    session.finish(serde_json::from_value(value).map_err(|e|e.to_string())?)?;
                },
                "abort" => {
                    let reason=value.as_str().filter(|r|!r.trim().is_empty()&&r.len()<=200).ok_or("中止理由を200文字以内で指定してください")?;
                    if session.active.is_none(){return Err("試行がありません".into());}
                    s.audio.stop();session.record("aborted",json!({"reason":reason,"primaryResponse":session.active.as_ref().and_then(|a|a.response.clone())}))?;
                    session.active=None;session.index+=1;
                },
                _ => return Err("操作が不正です".into()),
            }
            Ok(())
        })();
        if session.fault.is_some(){s.audio.stop();}
        result?;drop(guard);Ok(s.study.status())
    }).await.map_err(|e|e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    fn session() -> Session {
        let p = serde_json::from_str(include_str!("../tests/unity-project.json")).unwrap();
        let plan = experiment_set::Plan {
            template: "modality".into(),
            seed: 42,
            repetitions: 1,
            blocks: 1,
            participants: vec!["P001".into()],
            primary: "direction".into(),
            secondary: vec!["comfort".into()],
            conditions: experiment_set::defaults("modality").unwrap(),
        };
        let manifest =
            experiment_set::generate(&p, plan, Value::Null, std::path::Path::new(".")).unwrap();
        let path = std::env::temp_dir().join(format!(
            "aircue-study-test-{}-{}.jsonl",
            std::process::id(),
            unix_ms()
        ));
        Session {
            trials: manifest.trials.clone(),
            manifest,
            index: 0,
            log: File::create(&path).unwrap(),
            path: path.display().to_string(),
            id: "test".into(),
            calibrated: true,
            checked: 15,
            calibration: None,
            active: Some(Active {
                generation: 1,
                onset_ms: 200.,
                duration_ms: 1000.,
                anchor: None,
                response: None,
            }),
            fault: None,
        }
    }
    #[test]
    fn response_time_is_frozen_before_ratings_and_duplicate_answers_are_rejected() {
        let mut s = session();
        let now = Instant::now();
        assert!(s.respond("left", now).is_err());
        let status = audio::Status {
            playing: true,
            position_ms: 250.,
            error: None,
            mode: "study".into(),
            output: "test".into(),
            generation: 1,
        };
        s.observe(&status, now).unwrap();
        assert!(s.respond("invalid", now).is_err());
        s.respond("left", now + Duration::from_millis(100)).unwrap();
        assert_eq!(
            s.active.as_ref().unwrap().response.as_ref().unwrap()["reactionTimeMs"],
            150.
        );
        assert!(s.respond("right", now).is_err());
        assert!(s.finish(BTreeMap::new()).is_err());
        s.finish(BTreeMap::from([("comfort".into(), 7.)])).unwrap();
        assert_eq!(s.index, 1);
        let contents = std::fs::read_to_string(&s.path).unwrap();
        assert!(contents.contains("trialCompleted"));
        let path = s.path.clone();
        drop(s);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn interrupted_output_is_aborted_instead_of_accepted_as_a_response() {
        let mut s = session();
        let status = audio::Status {
            playing: false,
            position_ms: 50.,
            error: None,
            mode: "study".into(),
            output: "test".into(),
            generation: 1,
        };
        assert!(s.observe(&status, Instant::now()).is_err());
        assert!(s.active.is_none());
        assert!(!s.calibrated);
        assert_eq!(s.index, 1);
        let path = s.path.clone();
        drop(s);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn log_failure_latches_fault_without_consuming_a_trial() {
        let mut s = session();
        s.log = File::open(&s.path).unwrap();
        assert!(s.record("test", Value::Null).is_err());
        assert!(s.fault.is_some());
        assert!(!s.calibrated);
        assert_eq!(s.index, 0);
        assert!(s.respond("left", Instant::now()).is_err());
        let path = s.path.clone();
        drop(s);
        std::fs::remove_file(path).unwrap();
    }
}
