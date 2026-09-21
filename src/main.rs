#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod audio;
mod auditory;
mod experiment;
mod experiment_set;
mod model;
mod stimulus_export;
mod study;
mod unity_export;
use model::{Clip, Project, Settings, Wave};
use std::{path::PathBuf, sync::Mutex};
use tauri::{Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;
struct App {
    project: Mutex<Project>,
    audio: audio::Audio,
    experiment: experiment::Runtime,
    study: study::Runtime,
    dir: PathBuf,
    startup_error: Option<String>,
}
fn persist(dir: &std::path::Path, p: &Project) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let file = dir.join("project.json");
    let temp = dir.join("project.tmp");
    std::fs::write(
        &temp,
        serde_json::to_vec_pretty(p).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    if file.exists() {
        std::fs::copy(&file, dir.join("project.backup.json")).map_err(|e| e.to_string())?;
    }
    std::fs::rename(temp, file).map_err(|e| e.to_string())
}
fn change(
    app: &tauri::AppHandle,
    s: &App,
    f: impl FnOnce(&mut Project),
) -> Result<Project, String> {
    let _idle = s.study.idle()?;
    let mut locked = s.project.lock().map_err(|e| e.to_string())?;
    let mut next = locked.clone();
    f(&mut next);
    next.validate()?;
    persist(&s.dir, &next)?;
    *locked = next.clone();
    s.experiment.invalidate(app);
    app.emit("project-changed", &next)
        .map_err(|e| e.to_string())?;
    Ok(next)
}
#[tauri::command]
fn get_presets() -> Vec<Wave> {
    model::default_waves()
}
#[tauri::command]
fn get_project(s: State<App>) -> Project {
    s.project.lock().unwrap().clone()
}
#[tauri::command]
fn startup_error(s: State<App>) -> Option<String> {
    s.startup_error.clone()
}
#[tauri::command]
fn save_draft(app: tauri::AppHandle, s: State<App>, wave: Wave) -> Result<Project, String> {
    change(&app, &s, |p| p.draft = wave)
}
#[tauri::command]
fn save_wave(app: tauri::AppHandle, s: State<App>, wave: Wave) -> Result<Project, String> {
    change(&app, &s, |p| {
        if let Some(old) = p.waves.iter_mut().find(|w| w.id == wave.id) {
            *old = wave.clone()
        } else {
            p.waves.push(wave.clone())
        }
        p.draft = wave;
    })
}
#[tauri::command]
fn update_timeline(
    app: tauri::AppHandle,
    s: State<App>,
    clips: Vec<Clip>,
    duration_ms: f64,
) -> Result<Project, String> {
    change(&app, &s, |p| {
        p.clips = clips;
        p.duration_ms = duration_ms;
    })
}
#[tauri::command]
fn update_settings(
    app: tauri::AppHandle,
    s: State<App>,
    settings: Settings,
) -> Result<Project, String> {
    change(&app, &s, |p| {
        s.audio.stop();
        p.settings = settings;
    })
}
#[tauri::command]
async fn list_devices() -> Result<Vec<audio::DeviceInfo>, String> {
    tauri::async_runtime::spawn_blocking(audio::devices)
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn preview_wave(app: tauri::AppHandle, wave: Wave) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let s = app.state::<App>();
        let _idle = s.study.idle()?;
        let settings = s.project.lock().unwrap().settings.clone();
        let device = settings
            .device_id
            .clone()
            .ok_or("出力設定で機器を選択してください")?;
        let mono = wave.mono(false)?;
        let ch = settings.channel(&settings.preview_lane)?;
        let mut samples = vec![0.; mono.len() * 4];
        for (i, v) in mono.into_iter().enumerate() {
            samples[i * 4 + ch] = v;
        }
        s.audio.play(
            device,
            samples,
            model::db_gain(wave.level_db) as f32,
            true,
            [settings.channel("left")?, settings.channel("right")?],
        )
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn preview_timeline(app: tauri::AppHandle, target: Option<String>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let s = app.state::<App>();
        let _idle = s.study.idle()?;
        let p = s.project.lock().unwrap().clone();
        if p.clips.is_empty() && p.audio_clips.is_empty() {
            return Err("波形をタイムラインに配置してください".into());
        }
        let device = p
            .settings
            .device_id
            .clone()
            .ok_or("出力設定で機器を選択してください")?;
        let target = target.unwrap_or_else(|| "all".into());
        let headphones = target == "audio" || (target == "all" && p.clips.is_empty());
        let both = target == "all" && !p.clips.is_empty() && !p.audio_clips.is_empty();
        let pair = if headphones {
            [p.settings.routing[0] - 1, p.settings.routing[1] - 1]
        } else {
            [p.settings.channel("left")?, p.settings.channel("right")?]
        };
        s.audio.play_extended(
            device,
            auditory::render_project(&p, &s.dir, &target)?,
            1.,
            false,
            pair,
            "timeline",
            false,
            both,
        )
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
fn stop_audio(s: State<App>) -> Result<(), String> {
    s.study.end(&s.audio, "stopRequested")
}
#[tauri::command]
fn audio_status(s: State<App>) -> audio::Status {
    s.study.tick(&s.audio);
    s.audio.status()
}
#[tauri::command]
fn experiment_status(s: State<App>) -> experiment::Status {
    s.experiment.status(&s.dir)
}
#[tauri::command]
fn start_experiment_server(
    app: tauri::AppHandle,
    s: State<App>,
    port: u16,
) -> Result<experiment::Status, String> {
    let _idle = s.study.idle()?;
    s.experiment.start(app, port)
}
#[tauri::command]
fn stop_experiment_server(
    app: tauri::AppHandle,
    s: State<App>,
) -> Result<experiment::Status, String> {
    let _idle = s.study.idle()?;
    Ok(s.experiment.stop_server(&app))
}
#[tauri::command]
fn prepare_experiment(
    app: tauri::AppHandle,
    s: State<App>,
    target: String,
) -> Result<serde_json::Value, String> {
    s.experiment.prepare(&app, &target)
}
#[tauri::command]
fn play_experiment_test(
    app: tauri::AppHandle,
    s: State<App>,
    trial_id: String,
    delay_ms: u64,
) -> Result<serde_json::Value, String> {
    s.experiment.play(&app, &trial_id, delay_ms)
}
#[tauri::command]
fn stop_experiment_trial(
    app: tauri::AppHandle,
    s: State<App>,
) -> Result<serde_json::Value, String> {
    s.experiment.stop_trial(&app, None)
}
#[tauri::command]
fn experiment_template(template: String) -> Result<Vec<experiment_set::Condition>, String> {
    experiment_set::defaults(&template)
}
#[tauri::command]
async fn generate_experiment_set(
    app: tauri::AppHandle,
    plan: experiment_set::Plan,
) -> Result<experiment_set::Manifest, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let s = app.state::<App>();
        let p = s.project.lock().unwrap().clone();
        let device = audio::devices()?
            .into_iter()
            .find(|d| Some(&d.id) == p.settings.device_id.as_ref());
        experiment_set::generate(
            &p,
            plan,
            serde_json::to_value(device).map_err(|e| e.to_string())?,
            &s.dir,
        )
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn experiment_manifest_file(
    app: tauri::AppHandle,
    manifest: Option<experiment_set::Manifest>,
) -> Result<Option<experiment_set::Manifest>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let dialog = app
            .dialog()
            .file()
            .add_filter("AirCue Experiment", &["aircueexp"]);
        if let Some(m) = manifest {
            experiment_set::validate(&m)?;
            let Some(file) = dialog
                .set_file_name("Experiment.aircueexp")
                .blocking_save_file()
            else {
                return Ok(None);
            };
            std::fs::write(
                file.into_path().map_err(|e| e.to_string())?,
                serde_json::to_vec_pretty(&m).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            Ok(Some(m))
        } else {
            let Some(file) = dialog.blocking_pick_file() else {
                return Ok(None);
            };
            Ok(Some(experiment_set::load(
                &file.into_path().map_err(|e| e.to_string())?,
            )?))
        }
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn export_experiment_set(
    app: tauri::AppHandle,
    manifest: experiment_set::Manifest,
    unity: Option<bool>,
) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let s = app.state::<App>();
        let p = s.project.lock().unwrap().clone();
        let Some(folder) = app.dialog().file().blocking_pick_folder() else {
            return Ok(None);
        };
        experiment_set::export_with_adapter(
            &manifest,
            &p,
            &s.dir,
            &folder.into_path().map_err(|e| e.to_string())?,
            unity.unwrap_or(false),
        )
        .map(|p| Some(p.display().to_string()))
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn preview_calibration(
    app: tauri::AppHandle,
    modality: String,
    side: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        if !["audio", "air"].contains(&modality.as_str())
            || !["left", "right"].contains(&side.as_str())
        {
            return Err("左右と刺激を指定してください".into());
        }
        let s = app.state::<App>();
        let _idle = s.study.idle()?;
        let p = s.project.lock().unwrap().clone();
        let device = p
            .settings
            .device_id
            .clone()
            .ok_or("出力設定で機器を選択してください")?;
        let index = if modality == "audio" { 0 } else { 2 } + if side == "left" { 0 } else { 1 };
        let (_, rendered) = experiment_set::calibration(&p, &s.dir, index)?;
        let pair = if modality == "audio" {
            [p.settings.routing[0] - 1, p.settings.routing[1] - 1]
        } else {
            [p.settings.routing[2] - 1, p.settings.routing[3] - 1]
        };
        s.audio.play_extended(
            device,
            rendered,
            1.,
            false,
            pair,
            "calibration",
            false,
            false,
        )
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
fn set_preview_level(s: State<App>, db: f64) -> Result<(), String> {
    let _idle = s.study.idle()?;
    s.audio.set_level(db)
}
#[tauri::command]
async fn project_file(app: tauri::AppHandle, load: bool) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let dialog = app.dialog().file().add_filter("AirCue", &["aircue"]);
        let choice = if load {
            dialog.blocking_pick_file()
        } else {
            dialog
                .set_file_name("プロジェクト.aircue")
                .blocking_save_file()
        };
        let Some(path) = choice else { return Ok(None) };
        let path = path.into_path().map_err(|e| e.to_string())?;
        let s = app.state::<App>();
        if load {
            let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
            if meta.len() > 2_000_000 {
                return Err("ファイルが大きすぎます".into());
            }
            let mut p: Project =
                serde_json::from_slice(&std::fs::read(&path).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            p.validate()?;
            p.ensure_presets();
            auditory::copy_assets(&p, &path.with_extension("media"), &s.dir)?;
            p.settings.device_id = None;
            change(&app, &s, |old| {
                s.audio.stop();
                *old = p;
            })?;
        } else {
            let p = s.project.lock().unwrap();
            auditory::copy_assets(&p, &s.dir, &path.with_extension("media"))?;
            std::fs::write(
                &path,
                serde_json::to_vec_pretty(&*p).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
        }
        Ok(Some(path.display().to_string()))
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn export_wav(app: tauri::AppHandle, wave: Option<Wave>) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let (samples, channels) = if let Some(w) = wave {
            (w.mono(true)?, 1)
        } else {
            (
                auditory::render_project(
                    &app.state::<App>().project.lock().unwrap(),
                    &app.state::<App>().dir,
                    "all",
                )?,
                4,
            )
        };
        model::check_peak(&samples, 1.)?;
        let Some(path) = app
            .dialog()
            .file()
            .add_filter("WAV", &["wav"])
            .set_file_name(if channels == 1 {
                "波形.wav"
            } else {
                "タイムライン.wav"
            })
            .blocking_save_file()
        else {
            return Ok(None);
        };
        let path = path.into_path().map_err(|e| e.to_string())?;
        write_wav(&path, &samples, channels)?;
        Ok(Some(path.display().to_string()))
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn import_audio(app: tauri::AppHandle) -> Result<Option<Project>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let Some(file) = app
            .dialog()
            .file()
            .add_filter("音声", &["wav", "mp3", "flac", "ogg", "m4a", "aac", "aiff"])
            .blocking_pick_file()
        else {
            return Ok(None);
        };
        let path = file.into_path().map_err(|e| e.to_string())?;
        let s = app.state::<App>();
        if std::fs::metadata(&path).map_err(|e| e.to_string())?.len() > 128 * 1024 * 1024 {
            return Err("音声ファイルは128 MB以内にしてください".into());
        }
        let asset = auditory::import_file(&path, &s.dir)?;
        let design = auditory::Design {
            source: "file".into(),
            asset_id: Some(asset.id.clone()),
            duration_ms: (asset.frames as f64 * 1000. / model::RATE as f64).min(10000.),
            level_db: -18.,
            ..Default::default()
        };
        let p = change(&app, &s, |p| {
            if !p.audio_assets.iter().any(|a| a.id == asset.id) {
                p.audio_assets.push(asset);
            }
            p.audio_draft = design;
        })?;
        Ok(Some(p))
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
fn save_audio_draft(
    app: tauri::AppHandle,
    s: State<App>,
    design: auditory::Design,
) -> Result<Project, String> {
    change(&app, &s, |p| p.audio_draft = design)
}
#[tauri::command]
fn update_audio_timeline(
    app: tauri::AppHandle,
    s: State<App>,
    clips: Vec<auditory::AudioClip>,
    duration_ms: f64,
) -> Result<Project, String> {
    change(&app, &s, |p| {
        p.audio_clips = clips;
        p.duration_ms = duration_ms;
    })
}
#[tauri::command]
async fn preview_audio(
    app: tauri::AppHandle,
    design: auditory::Design,
    looping: bool,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let s = app.state::<App>();
        let _idle = s.study.idle()?;
        let p = s.project.lock().unwrap().clone();
        let device = p
            .settings
            .device_id
            .clone()
            .ok_or("出力設定で機器を選択してください")?;
        let stereo = auditory::render_design(&design, &p.audio_assets, &s.dir)?;
        let four = auditory::route_headphones(&stereo, p.settings.routing);
        s.audio.play_extended(
            device,
            four,
            1.,
            false,
            [p.settings.routing[0] - 1, p.settings.routing[1] - 1],
            "audio",
            looping,
            false,
        )
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn update_audio_preview(
    app: tauri::AppHandle,
    design: auditory::Design,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let s = app.state::<App>();
        let _idle = s.study.idle()?;
        let status = s.audio.status();
        if !status.playing || status.mode != "audio" {
            return Ok(());
        }
        let p = s.project.lock().unwrap().clone();
        let stereo = auditory::render_design(&design, &p.audio_assets, &s.dir)?;
        s.audio.replace_preview(
            status.generation,
            auditory::route_headphones(&stereo, p.settings.routing),
        )
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
fn set_audio_loop(s: State<App>, looping: bool) {
    s.audio.set_loop(looping)
}
#[tauri::command]
async fn export_audio(
    app: tauri::AppHandle,
    design: auditory::Design,
) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let s = app.state::<App>();
        let p = s.project.lock().unwrap().clone();
        let stereo = auditory::render_design(&design, &p.audio_assets, &s.dir)?;
        let Some(file) = app
            .dialog()
            .file()
            .add_filter("WAV", &["wav"])
            .set_file_name("オーディオ.wav")
            .blocking_save_file()
        else {
            return Ok(None);
        };
        let path = file.into_path().map_err(|e| e.to_string())?;
        write_wav(&path, &stereo, 2)?;
        Ok(Some(path.display().to_string()))
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn export_unity(app: tauri::AppHandle) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let s = app.state::<App>();
        let project = s.project.lock().unwrap().clone();
        let Some(folder) = app.dialog().file().blocking_pick_folder() else {
            return Ok(None);
        };
        let destination = folder.into_path().map_err(|e| e.to_string())?;
        let output = unity_export::export(&project, &s.dir, &destination)?;
        Ok(Some(output.display().to_string()))
    })
    .await
    .map_err(|e| e.to_string())?
}
fn write_wav(path: &std::path::Path, samples: &[f32], channels: u16) -> Result<(), String> {
    model::check_peak(samples, 1.)?;
    let spec = hound::WavSpec {
        channels,
        sample_rate: model::RATE,
        bits_per_sample: 24,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).map_err(|e| e.to_string())?;
    for sample in samples {
        writer
            .write_sample((sample * 8_388_607.).round() as i32)
            .map_err(|e| e.to_string())?;
    }
    writer.finalize().map_err(|e| e.to_string())
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--diagnostics") {
        if let Some(path) = args.get(2) {
            let result =
                serde_json::json!({"asio":cfg!(feature="asio"),"devices":audio::devices()});
            let _ = std::fs::write(path, serde_json::to_vec_pretty(&result).unwrap());
        }
        return;
    }
    if matches!(
        args.get(1).map(String::as_str),
        Some("--export-unity" | "--export-experiment" | "--export-experiment-unity")
    ) {
        let result = (|| -> Result<PathBuf, String> {
            let file = PathBuf::from(
                args.get(2)
                    .ok_or("Usage: AirCue --export-unity project.aircue destination")?,
            );
            let destination = PathBuf::from(args.get(3).ok_or("Export destination is required")?);
            if std::fs::metadata(&file).map_err(|e| e.to_string())?.len() > 2_000_000 {
                return Err("Project file is too large".into());
            }
            let project: Project =
                serde_json::from_slice(&std::fs::read(&file).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            let media = if file.extension().is_some_and(|e| e == "aircue") {
                file.with_extension("media")
            } else {
                file.parent().unwrap().to_path_buf()
            };
            if args[1].starts_with("--export-experiment") {
                let template = args.get(4).map(String::as_str).unwrap_or("modality");
                let plan = experiment_set::Plan {
                    template: template.into(),
                    seed: 42,
                    repetitions: 4,
                    blocks: 1,
                    participants: vec!["P001".into()],
                    primary: "direction".into(),
                    secondary: vec![],
                    conditions: experiment_set::defaults(template)?,
                };
                let manifest =
                    experiment_set::generate(&project, plan, serde_json::Value::Null, &media)?;
                if args[1] == "--export-experiment-unity" {
                    experiment_set::export_with_adapter(
                        &manifest,
                        &project,
                        &media,
                        &destination,
                        true,
                    )
                } else {
                    experiment_set::export(&manifest, &project, &media, &destination)
                }
            } else {
                unity_export::export(&project, &media, &destination)
            }
        })();
        match result {
            Ok(path) => println!("{}", path.display()),
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1)
            }
        }
        return;
    }
    let dir = std::env::var_os("AIRCUE_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::current_exe()
                .unwrap()
                .parent()
                .unwrap()
                .join("data")
        });
    let loaded = if dir.join("project.json").exists() {
        std::fs::read(dir.join("project.json"))
            .map_err(|e| e.to_string())
            .and_then(|b| serde_json::from_slice::<Project>(&b).map_err(|e| e.to_string()))
            .and_then(|mut p| {
                p.validate()?;
                p.ensure_presets();
                p.validate()?;
                Ok(p)
            })
    } else {
        Ok(Project::default())
    };
    let (project, startup_issue) = match loaded {
        Ok(p) => (p, None),
        Err(e) => {
            let backup = dir.join(format!(
                "project.invalid-{}.json",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis()
            ));
            let _ = std::fs::copy(dir.join("project.json"), backup);
            (
                Project::default(),
                Some(format!(
                    "前回のデータを読み込めませんでした。元ファイルを退避しました: {e}"
                )),
            )
        }
    };
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(App {
            project: Mutex::new(project),
            audio: audio::Audio::new(),
            experiment: experiment::Runtime::new(),
            study: study::Runtime::default(),
            dir,
            startup_error: startup_issue,
        })
        .invoke_handler(tauri::generate_handler![
            study::start_study,
            study::study_status,
            study::study_action,
            experiment_template,
            generate_experiment_set,
            experiment_manifest_file,
            export_experiment_set,
            preview_calibration,
            get_project,
            get_presets,
            import_audio,
            save_audio_draft,
            update_audio_timeline,
            preview_audio,
            update_audio_preview,
            export_audio,
            set_audio_loop,
            startup_error,
            save_draft,
            save_wave,
            update_timeline,
            update_settings,
            list_devices,
            preview_wave,
            preview_timeline,
            stop_audio,
            audio_status,
            experiment_status,
            start_experiment_server,
            stop_experiment_server,
            prepare_experiment,
            play_experiment_test,
            stop_experiment_trial,
            set_preview_level,
            project_file,
            export_wav,
            export_unity
        ])
        .on_window_event(|w, e| {
            if let tauri::WindowEvent::CloseRequested { .. } = e {
                let state = w.state::<App>();
                if let Err(e) = state.study.end(&state.audio, "windowClosed") {
                    eprintln!("{e}");
                }
                w.state::<App>().audio.stop();
                w.state::<App>().experiment.stop_server(w.app_handle());
                w.app_handle().exit(0);
            }
        })
        .run(tauri::generate_context!())
        .expect("AirCue");
}

#[cfg(test)]
mod storage_tests {
    use super::*;
    #[test]
    fn wav_export_preserves_24bit_four_channel_routing() {
        let path = std::env::temp_dir().join(format!("aircue-wav-{}.wav", std::process::id()));
        let mut p = Project::default();
        p.clips.push(Clip {
            id: "test".into(),
            wave_id: "single".into(),
            lane: "right".into(),
            start_ms: 100.,
            gain_db: 0.,
        });
        let rendered = p.render().unwrap();
        write_wav(&path, &rendered, 4).unwrap();
        let mut reader = hound::WavReader::open(&path).unwrap();
        assert_eq!(reader.spec().channels, 4);
        assert_eq!(reader.spec().bits_per_sample, 24);
        assert_eq!(reader.spec().sample_rate, 48000);
        let read: Vec<i32> = reader.samples().map(Result::unwrap).collect();
        assert_eq!(read.len(), rendered.len());
        for (actual, expected) in read.iter().zip(rendered.iter()) {
            assert!((*actual as f64 / 8_388_607. - *expected as f64).abs() < 0.0000002);
        }
        drop(reader);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn save_replaces_previous_project_and_keeps_backup() {
        let dir = std::env::temp_dir().join(format!(
            "aircue-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut p = Project::default();
        persist(&dir, &p).unwrap();
        p.draft.level_db = -24.;
        persist(&dir, &p).unwrap();
        let current: Project =
            serde_json::from_slice(&std::fs::read(dir.join("project.json")).unwrap()).unwrap();
        let backup: Project =
            serde_json::from_slice(&std::fs::read(dir.join("project.backup.json")).unwrap())
                .unwrap();
        assert_eq!(current.draft.level_db, -24.);
        assert_eq!(backup.draft.level_db, -18.);
        assert!(!dir.join("project.tmp").exists());
        std::fs::remove_file(dir.join("project.json")).unwrap();
        std::fs::remove_file(dir.join("project.backup.json")).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
}
