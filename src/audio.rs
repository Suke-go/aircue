use crate::model::{self, RATE};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::Serialize;
use std::sync::{
    atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering},
    mpsc, Arc, Mutex,
};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub host: String,
    pub channels: u16,
    pub usable: bool,
}
pub fn devices() -> Result<Vec<DeviceInfo>, String> {
    let mut list = vec![];
    for host_id in cpal::available_hosts() {
        let Ok(host) = cpal::host_from_id(host_id) else {
            continue;
        };
        let Ok(devices) = host.output_devices() else {
            continue;
        };
        let mut counts = std::collections::HashMap::<String, usize>::new();
        for d in devices {
            let name = d.name().unwrap_or_else(|_| "名称不明".into());
            let ordinal = counts.entry(name.clone()).or_default();
            let id = serde_json::to_string(&(host_id.name(), &name, *ordinal)).unwrap();
            *ordinal += 1;
            let configs = d
                .supported_output_configs()
                .map(|c| c.collect::<Vec<_>>())
                .unwrap_or_default();
            let default = d.default_output_config().ok();
            let channels = configs
                .iter()
                .map(|c| c.channels())
                .chain(default.iter().map(|c| c.channels()))
                .max()
                .unwrap_or(0);
            let usable = configs
                .iter()
                .any(|c| c.channels() > 0 && supported_format(c.sample_format()))
                || default
                    .as_ref()
                    .is_some_and(|c| c.channels() > 0 && supported_format(c.sample_format()));
            list.push(DeviceInfo {
                id,
                name,
                host: host_id.name().into(),
                channels,
                usable,
            });
        }
    }
    Ok(list)
}
fn find_device(id: &str) -> Result<cpal::Device, String> {
    let (host_name, name, ordinal): (String, String, usize) =
        serde_json::from_str(id).map_err(|_| "デバイスIDが不正です")?;
    let host_id = cpal::available_hosts()
        .into_iter()
        .find(|h| h.name() == host_name)
        .ok_or("音声バックエンドが利用できません")?;
    let host = cpal::host_from_id(host_id).map_err(|e| e.to_string())?;
    let device = host
        .output_devices()
        .map_err(|e| e.to_string())?
        .filter(|d| d.name().ok().as_deref() == Some(name.as_str()))
        .nth(ordinal)
        .ok_or("選択した出力機器が見つかりません。出力設定を更新してください")?;
    Ok(device)
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub playing: bool,
    pub position_ms: f64,
    pub error: Option<String>,
    pub mode: String,
    pub output: String,
    pub generation: usize,
}
struct Shared {
    playing: AtomicBool,
    stop: AtomicBool,
    frames: AtomicUsize,
    gain: AtomicU32,
    base_peak: AtomicU32,
    waveform: AtomicBool,
    error: Mutex<Option<String>>,
    output: Mutex<String>,
    mode: Mutex<String>,
    looping: AtomicBool,
    generation: AtomicUsize,
    pending: Mutex<Option<Vec<f32>>>,
    config: Mutex<(usize, u32, [usize; 2])>,
}
enum Command {
    Play {
        device: String,
        samples: Vec<f32>,
        gain: f32,
        waveform: bool,
        air_channels: [usize; 2],
        mode: String,
        looping: bool,
        require_four: bool,
        reply: mpsc::SyncSender<Result<(), String>>,
    },
    Stop,
}
pub struct Audio {
    sender: mpsc::Sender<Command>,
    shared: Arc<Shared>,
}
impl Audio {
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        let shared = Arc::new(Shared {
            playing: AtomicBool::new(false),
            stop: AtomicBool::new(false),
            frames: AtomicUsize::new(0),
            gain: AtomicU32::new(1f32.to_bits()),
            base_peak: AtomicU32::new(0),
            waveform: AtomicBool::new(false),
            error: Mutex::new(None),
            output: Mutex::new(String::new()),
            mode: Mutex::new("waveform".into()),
            looping: AtomicBool::new(false),
            generation: AtomicUsize::new(0),
            pending: Mutex::new(None),
            config: Mutex::new((4, RATE, [2, 3])),
        });
        let st = shared.clone();
        std::thread::Builder::new()
            .name("aircue-audio".into())
            .spawn(move || {
                let mut stream: Option<cpal::Stream> = None;
                loop {
                    match receiver.recv_timeout(std::time::Duration::from_millis(20)) {
                        Ok(Command::Stop) => {
                            st.stop.store(true, Ordering::Relaxed);
                        }
                        Ok(Command::Play {
                            device,
                            samples,
                            gain,
                            waveform,
                            air_channels,
                            mode,
                            looping,
                            require_four,
                            reply,
                        }) => {
                            drop(stream.take());
                            st.generation.fetch_add(1, Ordering::Relaxed);
                            *st.mode.lock().unwrap() = mode;
                            st.looping.store(looping, Ordering::Relaxed);
                            *st.pending.lock().unwrap() = None;
                            st.playing.store(false, Ordering::Relaxed);
                            st.stop.store(false, Ordering::Relaxed);
                            st.frames.store(0, Ordering::Relaxed);
                            st.gain.store(gain.to_bits(), Ordering::Relaxed);
                            st.base_peak
                                .store(model::peak(&samples).to_bits(), Ordering::Relaxed);
                            st.waveform.store(waveform, Ordering::Relaxed);
                            *st.error.lock().unwrap() = None;
                            *st.output.lock().unwrap() = String::new();
                            match start_stream(
                                &device,
                                samples,
                                air_channels,
                                require_four,
                                st.clone(),
                            ) {
                                Ok(s) => {
                                    stream = Some(s);
                                    let _ = reply.send(Ok(()));
                                }
                                Err(e) => {
                                    st.playing.store(false, Ordering::Relaxed);
                                    *st.error.lock().unwrap() = Some(e.clone());
                                    let _ = reply.send(Err(e));
                                }
                            }
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                    // Keep the completed stream alive, outputting silence, so queued hardware buffers can drain.
                }
            })
            .expect("audio worker");
        Self { sender, shared }
    }
    pub fn set_loop(&self, looping: bool) {
        if *self.shared.mode.lock().unwrap() == "audio" {
            self.shared.looping.store(looping, Ordering::Relaxed);
        }
    }
    pub fn play(
        &self,
        device: String,
        samples: Vec<f32>,
        gain: f32,
        waveform: bool,
        air_channels: [usize; 2],
    ) -> Result<(), String> {
        self.play_extended(
            device,
            samples,
            gain,
            waveform,
            air_channels,
            if waveform { "waveform" } else { "timeline" },
            false,
            false,
        )
    }
    pub fn play_extended(
        &self,
        device: String,
        samples: Vec<f32>,
        gain: f32,
        waveform: bool,
        air_channels: [usize; 2],
        mode: &str,
        looping: bool,
        require_four: bool,
    ) -> Result<(), String> {
        model::check_peak(&samples, gain)?;
        if samples.is_empty() || samples.len() % 4 != 0 {
            return Err("再生データが空または不正です".into());
        }
        if air_channels.iter().any(|ch| *ch >= 4) || air_channels[0] == air_channels[1] {
            return Err("空気砲のチャンネル割り当てが不正です".into());
        }
        let (tx, rx) = mpsc::sync_channel(1);
        self.sender
            .send(Command::Play {
                device,
                samples,
                gain,
                waveform,
                air_channels,
                mode: mode.into(),
                looping,
                require_four,
                reply: tx,
            })
            .map_err(|e| e.to_string())?;
        rx.recv().map_err(|e| e.to_string())?
    }
    pub fn replace_preview(&self, generation: usize, samples: Vec<f32>) -> Result<(), String> {
        model::check_peak(&samples, 1.)?;
        let (channels, rate, pair) = *self.shared.config.lock().unwrap();
        let next = prepare_output(&samples, channels, rate, pair);
        let mut pending = self.shared.pending.lock().unwrap();
        if self.shared.generation.load(Ordering::Relaxed) == generation
            && self.shared.playing.load(Ordering::Relaxed)
            && *self.shared.mode.lock().unwrap() == "audio"
        {
            *pending = Some(next);
        }
        Ok(())
    }
    pub fn stop(&self) {
        self.shared.generation.fetch_add(1, Ordering::Relaxed);
        self.shared.stop.store(true, Ordering::Relaxed);
        let _ = self.sender.send(Command::Stop);
    }
    pub fn set_level(&self, db: f64) -> Result<(), String> {
        model::range(db, -60., 0., "出力レベル")?;
        if self.shared.waveform.load(Ordering::Relaxed)
            && self.shared.playing.load(Ordering::Relaxed)
        {
            let gain = model::db_gain(db) as f32;
            let peak = f32::from_bits(self.shared.base_peak.load(Ordering::Relaxed));
            if peak * gain > 1.000001 {
                return Err("再生中の波形が0 dBFSを超えるため変更できません".into());
            }
            self.shared.gain.store(gain.to_bits(), Ordering::Relaxed);
        }
        Ok(())
    }
    pub fn status(&self) -> Status {
        Status {
            playing: self.shared.playing.load(Ordering::Relaxed),
            position_ms: self.shared.frames.load(Ordering::Relaxed) as f64 * 1000. / RATE as f64,
            error: self.shared.error.lock().unwrap().clone(),
            output: self.shared.output.lock().unwrap().clone(),
            mode: self.shared.mode.lock().unwrap().clone(),
            generation: self.shared.generation.load(Ordering::Relaxed),
        }
    }
}
fn supported_format(format: cpal::SampleFormat) -> bool {
    matches!(
        format,
        cpal::SampleFormat::F32
            | cpal::SampleFormat::I16
            | cpal::SampleFormat::I32
            | cpal::SampleFormat::U16
    )
}
fn output_options(
    ranges: Vec<cpal::SupportedStreamConfigRange>,
    default: Option<cpal::SupportedStreamConfig>,
) -> Vec<cpal::SupportedStreamConfig> {
    let mut options: Vec<_> = ranges
        .into_iter()
        .filter(|c| c.channels() > 0 && supported_format(c.sample_format()))
        .map(|c| {
            let rate = RATE.clamp(c.min_sample_rate().0, c.max_sample_rate().0);
            c.with_sample_rate(cpal::SampleRate(rate))
        })
        .collect();
    if let Some(c) = default {
        if c.channels() > 0 && supported_format(c.sample_format()) {
            options.push(c);
        }
    }
    options.sort_by_key(|c| {
        (
            c.channels() < 4,
            c.sample_rate().0.abs_diff(RATE),
            c.channels(),
        )
    });
    options.dedup_by(|a, b| a.config() == b.config() && a.sample_format() == b.sample_format());
    options
}
// Keep the project and WAV routing at four channels. Only device playback is adapted.
fn prepare_output(samples: &[f32], channels: usize, rate: u32, air: [usize; 2]) -> Vec<f32> {
    let source_frames = samples.len() / 4;
    let frames = (source_frames as f64 * rate as f64 / RATE as f64).round() as usize;
    let mut output = vec![0.; frames * channels];
    for (i, frame) in output.chunks_exact_mut(channels).enumerate() {
        let pos = i as f64 * RATE as f64 / rate as f64;
        let left = (pos.floor() as usize).min(source_frames - 1);
        let right = (left + 1).min(source_frames - 1);
        let frac = (pos - left as f64) as f32;
        let sample =
            |ch: usize| samples[left * 4 + ch] * (1. - frac) + samples[right * 4 + ch] * frac;
        if channels >= 4 {
            for ch in 0..4 {
                frame[ch] = sample(ch);
            }
        } else if channels >= 2 {
            frame[0] = sample(air[0]);
            frame[1] = sample(air[1]);
        } else {
            frame[0] = (sample(air[0]) + sample(air[1])) * 0.5;
        }
    }
    output
}
fn start_stream(
    id: &str,
    samples: Vec<f32>,
    air: [usize; 2],
    require_four: bool,
    shared: Arc<Shared>,
) -> Result<cpal::Stream, String> {
    let d = find_device(id)?;
    let ranges = d
        .supported_output_configs()
        .map(|c| c.collect())
        .unwrap_or_default();
    let options = output_options(ranges, d.default_output_config().ok());
    let mut last_error = String::from("機器から再生形式を取得できませんでした");
    for option in options {
        if require_four && option.channels() < 4 {
            last_error="オーディオと空気砲の同時出力には4chが必要です。再生対象をどちらか一方に切り替えてください".into();
            continue;
        }
        let format = option.sample_format();
        let config = option.config();
        *shared.config.lock().unwrap() = (config.channels as usize, config.sample_rate.0, air);
        let adapted = prepare_output(
            &samples,
            config.channels as usize,
            config.sample_rate.0,
            air,
        );
        let result = match format {
            cpal::SampleFormat::F32 => build::<f32>(&d, &config, adapted, shared.clone()),
            cpal::SampleFormat::I16 => build::<i16>(&d, &config, adapted, shared.clone()),
            cpal::SampleFormat::I32 => build::<i32>(&d, &config, adapted, shared.clone()),
            cpal::SampleFormat::U16 => build::<u16>(&d, &config, adapted, shared.clone()),
            _ => continue,
        };
        match result {
            Ok(stream) => {
                shared.playing.store(true, Ordering::Relaxed);
                if let Err(e) = stream.play() {
                    shared.playing.store(false, Ordering::Relaxed);
                    last_error = e.to_string();
                    continue;
                }
                let mode = if config.channels >= 4 {
                    "チャンネル割り当て通り"
                } else if config.channels >= 2 {
                    "互換再生：選択した信号の左・右 → 機器のL/R"
                } else {
                    "互換再生：選択した信号の左右を合成"
                };
                *shared.output.lock().unwrap() = format!(
                    "{mode} · {} ch · {} Hz",
                    config.channels, config.sample_rate.0
                );
                *shared.error.lock().unwrap() = None;
                return Ok(stream);
            }
            Err(e) => last_error = e,
        }
    }
    Err(format!("選択した出力を開始できませんでした: {last_error}"))
}
fn build<T: cpal::SizedSample + cpal::FromSample<f32>>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    samples: Vec<f32>,
    shared: Arc<Shared>,
) -> Result<cpal::Stream, String> {
    let channels = config.channels as usize;
    let mut samples = samples;
    let mut total = samples.len() / channels;
    let mut transition: Option<(Vec<f32>, usize, usize)> = None;
    let sample_rate = config.sample_rate.0;
    let mut cursor = 0;
    let mut current_gain = f32::from_bits(shared.gain.load(Ordering::Relaxed));
    let mut stop_frames = 0usize;
    let err = shared.clone();
    device
        .build_output_stream(
            config,
            move |output: &mut [T], _: &cpal::OutputCallbackInfo| {
                if let Ok(mut pending) = shared.pending.try_lock() {
                    if let Some(next) = pending.take() {
                        let old = std::mem::replace(&mut samples, next);
                        transition = Some((old, cursor, 0));
                        total = samples.len() / channels;
                        cursor %= total;
                    }
                }
                let looping = shared.looping.load(Ordering::Relaxed);
                let target = f32::from_bits(shared.gain.load(Ordering::Relaxed));
                for frame in output.chunks_mut(channels) {
                    current_gain += (target - current_gain) * 0.02;
                    if shared.stop.load(Ordering::Relaxed) {
                        stop_frames = (stop_frames + 1).min(96);
                    }
                    let fade = 1. - stop_frames as f32 / 96.;
                    for (ch, out) in frame.iter_mut().enumerate() {
                        let mut v = if cursor < total {
                            samples[cursor * channels + ch]
                        } else {
                            0.
                        };
                        if let Some((old, pos, step)) = &transition {
                            let old_frames = old.len() / channels;
                            let old_pos = if looping { pos % old_frames } else { *pos };
                            let before = if old_pos < old_frames {
                                old[old_pos * channels + ch]
                            } else {
                                0.
                            };
                            let alpha = (*step + 1) as f32 / 256.;
                            v = before * (1. - alpha) + v * alpha;
                        }
                        *out = T::from_sample((v * current_gain * fade).clamp(-1., 1.));
                    }
                    if let Some((_, pos, step)) = &mut transition {
                        *pos += 1;
                        *step += 1;
                        if *step >= 256 {
                            transition = None;
                        }
                    }
                    if cursor < total {
                        cursor += 1;
                    }
                    if cursor >= total && looping && stop_frames == 0 {
                        cursor = 0;
                    }
                    if stop_frames == 96 {
                        cursor = total;
                    }
                }
                shared.frames.store(
                    (cursor as u64 * RATE as u64 / sample_rate as u64) as usize,
                    Ordering::Relaxed,
                );
                if cursor >= total {
                    shared.playing.store(false, Ordering::Relaxed);
                }
            },
            move |e| {
                *err.error.lock().unwrap() = Some(e.to_string());
                err.playing.store(false, Ordering::Relaxed);
            },
            None,
        )
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn range(channels: u16, min: u32, max: u32) -> cpal::SupportedStreamConfigRange {
        cpal::SupportedStreamConfigRange::new(
            channels,
            cpal::SampleRate(min),
            cpal::SampleRate(max),
            cpal::SupportedBufferSize::Unknown,
            cpal::SampleFormat::F32,
        )
    }
    #[test]
    fn stereo_44100_device_is_accepted() {
        let configs = output_options(vec![range(2, 44100, 44100)], None);
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].channels(), 2);
        assert_eq!(configs[0].sample_rate().0, 44100);
    }
    #[test]
    fn four_channels_are_preferred_and_default_is_a_fallback() {
        let default = range(2, 44100, 44100).with_sample_rate(cpal::SampleRate(44100));
        let configs = output_options(
            vec![range(2, 48000, 48000), range(4, 96000, 96000)],
            Some(default),
        );
        assert_eq!(configs[0].channels(), 4);
        assert_eq!(configs[1].sample_rate().0, 48000);
        assert_eq!(configs[2].sample_rate().0, 44100);
        assert_eq!(
            output_options(
                vec![],
                Some(range(2, 48000, 48000).with_sample_rate(cpal::SampleRate(48000)))
            )
            .len(),
            1
        );
    }
    #[test]
    fn stereo_preserves_air_sides_for_any_project_routing() {
        let samples = vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8];
        assert_eq!(
            prepare_output(&samples, 2, RATE, [2, 3]),
            vec![0.3, 0.4, 0.7, 0.8]
        );
        assert_eq!(
            prepare_output(&samples, 2, RATE, [1, 0]),
            vec![0.2, 0.1, 0.6, 0.5]
        );
        assert_eq!(prepare_output(&samples, 4, RATE, [2, 3]), samples);
        assert_eq!(
            prepare_output(&samples, 6, RATE, [2, 3]),
            vec![0.1, 0.2, 0.3, 0.4, 0., 0., 0.5, 0.6, 0.7, 0.8, 0., 0.]
        );
    }
    #[test]
    fn resampling_preserves_duration_frequency_and_peak() {
        let mut samples = vec![0.; 3840 * 4];
        for (i, f) in samples.chunks_exact_mut(4).enumerate() {
            f[2] = (2. * std::f32::consts::PI * 40. * i as f32 / RATE as f32).sin() * 0.4;
        }
        for rate in [44100, 96000] {
            let out = prepare_output(&samples, 2, rate, [2, 3]);
            assert_eq!(out.len(), (0.08 * rate as f64).round() as usize * 2);
            assert!(model::peak(&out) <= 0.4);
            for (i, f) in out.chunks_exact(2).enumerate().take(out.len() / 2 - 2) {
                let expected =
                    (2. * std::f32::consts::PI * 40. * i as f32 / rate as f32).sin() * 0.4;
                assert!((f[0] - expected).abs() < 0.00001);
                assert_eq!(f[1], 0.);
            }
        }
    }
    #[test]
    fn mono_mix_does_not_double_the_peak() {
        assert_eq!(
            prepare_output(&[0., 0., 0.8, 0.8], 1, RATE, [2, 3]),
            vec![0.8]
        );
    }
}
