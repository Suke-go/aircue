use crate::{auditory, model, App};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::VecDeque,
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Manager};

const MIN_PORT: u16 = 1024;
const MAX_LINE: usize = 16 * 1024;
const MAX_EVENTS: usize = 200;

#[derive(Clone)]
struct Prepared {
    samples: Vec<f32>,
    target: String,
    project_hash: String,
    duration_ms: f64,
    device_id: String,
    output: String,
    pair: [usize; 2],
    require_four: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    sequence: u64,
    event: String,
    unix_ms: u64,
    trial_id: Option<String>,
    cue_id: Option<String>,
    target: Option<String>,
    scheduled_unix_ms: Option<u64>,
    position_ms: Option<f64>,
    detail: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    running: bool,
    host: &'static str,
    port: u16,
    session_token: String,
    prepared: bool,
    cue_id: Option<&'static str>,
    target: Option<String>,
    project_hash: Option<String>,
    duration_ms: Option<f64>,
    device_id: Option<String>,
    output: Option<String>,
    active_trial_id: Option<String>,
    events: Vec<Event>,
    log_path: String,
}

struct State {
    running: bool,
    port: u16,
    token: String,
    stop_server: Option<Arc<AtomicBool>>,
    prepared: Option<Prepared>,
    active_trial_id: Option<String>,
    events: VecDeque<Event>,
    next_sequence: u64,
}

pub struct Runtime {
    state: Mutex<State>,
    log_lock: Mutex<()>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    command: String,
    request_id: String,
    token: String,
    #[serde(default)]
    trial_id: Option<String>,
    #[serde(default)]
    cue_id: Option<String>,
    #[serde(default)]
    target: Option<String>,
    #[serde(default)]
    delay_ms: Option<u64>,
}

impl Runtime {
    pub fn is_running(&self) -> bool {
        self.state.lock().unwrap().running
    }
    pub fn new() -> Self {
        Self {
            state: Mutex::new(State {
                running: false,
                port: 39100,
                token: String::new(),
                stop_server: None,
                prepared: None,
                active_trial_id: None,
                events: VecDeque::new(),
                next_sequence: 1,
            }),
            log_lock: Mutex::new(()),
        }
    }

    pub fn status(&self, dir: &std::path::Path) -> Status {
        let state = self.state.lock().unwrap();
        Status {
            running: state.running,
            host: "127.0.0.1",
            port: state.port,
            session_token: state.token.clone(),
            prepared: state.prepared.is_some(),
            cue_id: state.prepared.as_ref().map(|_| "current"),
            target: state.prepared.as_ref().map(|p| p.target.clone()),
            project_hash: state.prepared.as_ref().map(|p| p.project_hash.clone()),
            duration_ms: state.prepared.as_ref().map(|p| p.duration_ms),
            device_id: state.prepared.as_ref().map(|p| p.device_id.clone()),
            output: state.prepared.as_ref().map(|p| p.output.clone()),
            active_trial_id: state.active_trial_id.clone(),
            events: state.events.iter().cloned().collect(),
            log_path: dir.join("experiment-events.jsonl").display().to_string(),
        }
    }

    pub fn invalidate(&self, app: &AppHandle) {
        let changed = {
            let mut state = self.state.lock().unwrap();
            state.prepared.take().is_some()
        };
        if changed {
            self.record(
                app,
                "invalidated",
                None,
                Some("current".into()),
                None,
                None,
                None,
                Some("projectChanged".into()),
            );
        }
    }

    pub fn start(&self, app: AppHandle, port: u16) -> Result<Status, String> {
        if port < MIN_PORT {
            return Err(format!("ポートは{MIN_PORT}〜65535で指定してください"));
        }
        if self.state.lock().unwrap().running {
            return Err("実験連携はすでに開始しています".into());
        }
        let token = make_token()?;
        let listener = TcpListener::bind(("127.0.0.1", port))
            .map_err(|e| format!("127.0.0.1:{port}を開始できません: {e}"))?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let stop = Arc::new(AtomicBool::new(false));
        {
            let mut state = self.state.lock().unwrap();
            state.running = true;
            state.port = port;
            state.token = token;
            state.stop_server = Some(stop.clone());
            state.prepared = None;
            state.active_trial_id = None;
        }
        let thread_app = app.clone();
        if let Err(error) = std::thread::Builder::new()
            .name("aircue-experiment-server".into())
            .spawn(move || server_loop(listener, stop, thread_app))
        {
            let mut state = self.state.lock().unwrap();
            state.running = false;
            state.token.clear();
            state.stop_server = None;
            return Err(error.to_string());
        }
        self.record(&app, "serverStarted", None, None, None, None, None, None);
        Ok(self.status(&app.state::<App>().dir))
    }

    pub fn stop_server(&self, app: &AppHandle) -> Status {
        let trial = {
            let mut state = self.state.lock().unwrap();
            if let Some(stop) = state.stop_server.take() {
                stop.store(true, Ordering::Relaxed);
            }
            state.running = false;
            state.prepared = None;
            state.active_trial_id.take()
        };
        app.state::<App>().audio.stop();
        self.record(app, "serverStopped", trial, None, None, None, None, None);
        self.status(&app.state::<App>().dir)
    }

    pub fn prepare(&self, app: &AppHandle, target: &str) -> Result<Value, String> {
        let app_state = app.state::<App>();
        let _idle = app_state.study.idle()?;
        if !self.state.lock().unwrap().running {
            return Err("先に実験連携を開始してください".into());
        }
        if !["all", "audio", "air"].contains(&target) {
            return Err("再生対象はall、audio、airのいずれかです".into());
        }
        let state = app.state::<App>();
        let project = state.project.lock().map_err(|e| e.to_string())?.clone();
        if project.clips.is_empty() && project.audio_clips.is_empty() {
            return Err("タイムラインに音声または波形を配置してください".into());
        }
        let device_id = project
            .settings
            .device_id
            .clone()
            .ok_or("出力設定で機器を選択してください")?;
        let rendered = auditory::render_project(&project, &state.dir, target)?;
        let headphones = target == "audio" || (target == "all" && project.clips.is_empty());
        let both = target == "all" && !project.clips.is_empty() && !project.audio_clips.is_empty();
        let output = if target == "audio" || (target == "all" && project.clips.is_empty()) {
            format!(
                "イヤホン {} / {} ch",
                project.settings.routing[0], project.settings.routing[1]
            )
        } else if target == "air" || (target == "all" && project.audio_clips.is_empty()) {
            format!(
                "空気砲 {} / {} ch",
                project.settings.routing[2], project.settings.routing[3]
            )
        } else {
            format!(
                "イヤホン {} / {} ch・空気砲 {} / {} ch",
                project.settings.routing[0],
                project.settings.routing[1],
                project.settings.routing[2],
                project.settings.routing[3]
            )
        };
        let pair = if headphones {
            [
                project.settings.routing[0] - 1,
                project.settings.routing[1] - 1,
            ]
        } else {
            [
                project.settings.channel("left")?,
                project.settings.channel("right")?,
            ]
        };
        let encoded = serde_json::to_vec(&project).map_err(|e| e.to_string())?;
        let project_hash = format!("{:x}", Sha256::digest(encoded));
        let prepared = Prepared {
            samples: rendered,
            target: target.into(),
            project_hash: project_hash.clone(),
            duration_ms: project.duration_ms,
            device_id: device_id.clone(),
            output: output.clone(),
            pair,
            require_four: both,
        };
        self.state.lock().unwrap().prepared = Some(prepared);
        self.record(
            app,
            "prepared",
            None,
            Some("current".into()),
            Some(target.into()),
            None,
            None,
            Some(project_hash.clone()),
        );
        Ok(json!({
            "type":"ready","cueId":"current","target":target,
            "projectHash":project_hash,"durationMs":project.duration_ms,
            "deviceId":device_id,"output":output
        }))
    }

    pub fn play(&self, app: &AppHandle, trial_id: &str, delay_ms: u64) -> Result<Value, String> {
        let app_state = app.state::<App>();
        let _idle = app_state.study.idle()?;
        validate_id(trial_id, "trialId")?;
        if !(50..=5000).contains(&delay_ms) {
            return Err("delayMsは50〜5000 msで指定してください".into());
        }
        let prepared = self
            .state
            .lock()
            .unwrap()
            .prepared
            .clone()
            .ok_or("先にprepareで現在のタイムラインを準備してください")?;
        let delay_frames = model::ms_frame(delay_ms as f64);
        let samples = with_delay(&prepared.samples, delay_frames);
        let app_state = app.state::<App>();
        app_state.audio.play_extended(
            prepared.device_id.clone(),
            samples,
            1.,
            false,
            prepared.pair,
            "experiment",
            false,
            prepared.require_four,
        )?;
        let position = app_state.audio.status().position_ms;
        let remaining = (delay_ms as f64 - position).max(0.).round() as u64;
        let scheduled = unix_ms().saturating_add(remaining);
        self.state.lock().unwrap().active_trial_id = Some(trial_id.into());
        self.record(
            app,
            "scheduled",
            Some(trial_id.into()),
            Some("current".into()),
            Some(prepared.target.clone()),
            Some(scheduled),
            Some(position),
            Some(prepared.project_hash.clone()),
        );
        monitor_trial(app.clone(), trial_id.into(), delay_ms as f64, scheduled);
        Ok(json!({
            "type":"scheduled","trialId":trial_id,"cueId":"current",
            "target":prepared.target,"scheduledUnixMs":scheduled,
            "projectHash":prepared.project_hash,"durationMs":prepared.duration_ms,
            "deviceId":prepared.device_id,"output":prepared.output
        }))
    }

    pub fn stop_trial(
        &self,
        app: &AppHandle,
        requested_trial: Option<&str>,
    ) -> Result<Value, String> {
        let app_state = app.state::<App>();
        let _idle = app_state.study.idle()?;
        app.state::<App>().audio.stop();
        let trial = self.state.lock().unwrap().active_trial_id.take();
        self.record(
            app,
            "stopped",
            trial.clone().or_else(|| requested_trial.map(str::to_owned)),
            Some("current".into()),
            None,
            None,
            None,
            None,
        );
        Ok(json!({"type":"stopped","trialId":trial}))
    }

    fn record(
        &self,
        app: &AppHandle,
        event: &str,
        trial_id: Option<String>,
        cue_id: Option<String>,
        target: Option<String>,
        scheduled_unix_ms: Option<u64>,
        position_ms: Option<f64>,
        detail: Option<String>,
    ) {
        let item = {
            let mut state = self.state.lock().unwrap();
            let item = Event {
                sequence: state.next_sequence,
                event: event.into(),
                unix_ms: unix_ms(),
                trial_id,
                cue_id,
                target,
                scheduled_unix_ms,
                position_ms,
                detail,
            };
            state.next_sequence += 1;
            state.events.push_front(item.clone());
            state.events.truncate(MAX_EVENTS);
            item
        };
        let _log_guard = self.log_lock.lock().unwrap();
        let dir = &app.state::<App>().dir;
        if std::fs::create_dir_all(dir).is_ok() {
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(dir.join("experiment-events.jsonl"))
            {
                if let Ok(line) = serde_json::to_string(&item) {
                    let _ = writeln!(file, "{line}");
                }
            }
        }
    }
}

fn monitor_trial(app: AppHandle, trial_id: String, delay_ms: f64, scheduled: u64) {
    std::thread::spawn(move || {
        let generation = app.state::<App>().audio.status().generation;
        let mut started = false;
        loop {
            std::thread::sleep(Duration::from_millis(2));
            let status = app.state::<App>().audio.status();
            if status.generation != generation || status.mode != "experiment" {
                break;
            }
            if !started && status.position_ms >= delay_ms {
                started = true;
                app.state::<App>().experiment.record(
                    &app,
                    "started",
                    Some(trial_id.clone()),
                    Some("current".into()),
                    None,
                    Some(scheduled),
                    Some(status.position_ms - delay_ms),
                    None,
                );
            }
            if !status.playing {
                if started {
                    app.state::<App>().experiment.record(
                        &app,
                        "finished",
                        Some(trial_id.clone()),
                        Some("current".into()),
                        None,
                        Some(scheduled),
                        Some((status.position_ms - delay_ms).max(0.)),
                        status.error,
                    );
                }
                let app_state = app.state::<App>();
                let mut state = app_state.experiment.state.lock().unwrap();
                if state.active_trial_id.as_deref() == Some(&trial_id) {
                    state.active_trial_id = None;
                }
                break;
            }
        }
    });
}

fn server_loop(listener: TcpListener, stop: Arc<AtomicBool>, app: AppHandle) {
    while !stop.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _)) => {
                let connection_app = app.clone();
                std::thread::spawn(move || {
                    serve_connection(stream, |request| handle_request(&connection_app, request))
                });
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(_) => break,
        }
    }
    app.state::<App>().experiment.state.lock().unwrap().running = false;
}

fn serve_connection(mut stream: TcpStream, handler: impl Fn(Request) -> Value) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
    let Ok(reader_stream) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(reader_stream);
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) if line.len() > MAX_LINE => {
                let _ = write_response(&mut stream, json!({"ok":false,"error":"requestTooLarge"}));
                break;
            }
            Ok(_) => {
                let response = match serde_json::from_str::<Request>(line.trim()) {
                    Ok(request) => handler(request),
                    Err(_) => json!({"ok":false,"error":"invalidRequest"}),
                };
                if write_response(&mut stream, response).is_err() {
                    break;
                }
            }
        }
    }
}

fn handle_request(app: &AppHandle, request: Request) -> Value {
    if validate_id(&request.request_id, "requestId").is_err() {
        return json!({"ok":false,"requestId":request.request_id,"error":"invalidRequestId"});
    }
    let runtime = &app.state::<App>().experiment;
    let expected = runtime.state.lock().unwrap().token.clone();
    if request.token != expected || expected.is_empty() {
        return json!({"ok":false,"requestId":request.request_id,"error":"unauthorized"});
    }
    let result = match request.command.as_str() {
        "status" => {
            serde_json::to_value(runtime.status(&app.state::<App>().dir)).map_err(|e| e.to_string())
        }
        "prepare" => {
            if request.cue_id.as_deref().unwrap_or("current") != "current" {
                Err("cueIdはcurrentのみ使用できます".into())
            } else {
                runtime.prepare(app, request.target.as_deref().unwrap_or("all"))
            }
        }
        "play" => runtime.play(
            app,
            request.trial_id.as_deref().unwrap_or(""),
            request.delay_ms.unwrap_or(100),
        ),
        "stop" => runtime.stop_trial(app, request.trial_id.as_deref()),
        _ => Err("unknownCommand".into()),
    };
    match result {
        Ok(value) => json!({"ok":true,"requestId":request.request_id,"result":value}),
        Err(error) => json!({"ok":false,"requestId":request.request_id,"error":error}),
    }
}

fn write_response(stream: &mut TcpStream, value: Value) -> std::io::Result<()> {
    serde_json::to_writer(&mut *stream, &value)?;
    stream.write_all(b"\n")?;
    stream.flush()
}

fn validate_id(value: &str, name: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 120
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b':'))
    {
        return Err(format!(
            "{name}は1〜120文字の英数字・-_.:で指定してください"
        ));
    }
    Ok(())
}

fn with_delay(samples: &[f32], delay_frames: usize) -> Vec<f32> {
    let mut scheduled = vec![0.; delay_frames * 4];
    scheduled.extend_from_slice(samples);
    scheduled
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn make_token() -> Result<String, String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|e| format!("セッショントークンを生成できません: {e}"))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};

    #[test]
    fn ids_and_protocol_fields_are_strict() {
        assert!(validate_id("trial-01:block_A", "trialId").is_ok());
        assert!(validate_id("", "trialId").is_err());
        assert!(validate_id("日本語", "trialId").is_err());
        assert!(serde_json::from_str::<Request>(
            r#"{"command":"status","requestId":"1","token":"x","extra":1}"#
        )
        .is_err());
    }

    #[test]
    fn newline_protocol_supports_multiple_requests_and_rejects_bad_json() {
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            serve_connection(stream, |request| json!({"ok":true,"id":request.request_id}));
        });
        let mut client = TcpStream::connect(address).unwrap();
        client
            .write_all(b"{bad}\n{\"command\":\"status\",\"requestId\":\"r2\",\"token\":\"x\"}\n")
            .unwrap();
        client.shutdown(std::net::Shutdown::Write).unwrap();
        let lines: Vec<_> = BufReader::new(client).lines().map(Result::unwrap).collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(
            serde_json::from_str::<Value>(&lines[0]).unwrap()["error"],
            "invalidRequest"
        );
        assert_eq!(
            serde_json::from_str::<Value>(&lines[1]).unwrap()["id"],
            "r2"
        );
        server.join().unwrap();
    }

    #[test]
    fn delay_is_inserted_as_four_channel_silence_without_changing_the_cue() {
        let cue = vec![0.1, 0.2, 0.3, 0.4, -0.1, -0.2, -0.3, -0.4];
        let scheduled = with_delay(&cue, 3);
        assert_eq!(scheduled.len(), cue.len() + 12);
        assert!(scheduled[..12].iter().all(|sample| *sample == 0.));
        assert_eq!(&scheduled[12..], cue);
    }
}
