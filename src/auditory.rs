use crate::model::{self, Project, RATE};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};
use symphonia::core::{
    audio::SampleBuffer, codecs::DecoderOptions, errors::Error, formats::FormatOptions,
    io::MediaSourceStream, meta::MetadataOptions, probe::Hint,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Asset {
    pub id: String,
    pub name: String,
    pub frames: usize,
    pub source_channels: usize,
    pub peaks: Vec<f32>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Keyframe {
    pub at: f64,
    pub azimuth: f64,
    pub elevation: f64,
    pub distance_m: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Design {
    pub source: String,
    pub asset_id: Option<String>,
    pub frequency: f64,
    pub end_frequency: f64,
    pub offset_ms: f64,
    pub duration_ms: f64,
    pub level_db: f64,
    pub spatial: bool,
    pub azimuth: f64,
    pub elevation: f64,
    pub distance_m: f64,
    pub motion: String,
    pub speed: f64,
    pub end_azimuth: f64,
    #[serde(default)]
    pub keyframes: Vec<Keyframe>,
}
impl Default for Design {
    fn default() -> Self {
        Self {
            source: "noise".into(),
            asset_id: None,
            frequency: 440.,
            end_frequency: 4000.,
            offset_ms: 0.,
            duration_ms: 3000.,
            level_db: -24.,
            spatial: false,
            azimuth: 0.,
            elevation: 0.,
            distance_m: 1.,
            motion: "fixed".into(),
            speed: 45.,
            end_azimuth: 90.,
            keyframes: Vec::new(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AudioClip {
    pub id: String,
    pub name: String,
    pub start_ms: f64,
    pub gain_db: f64,
    pub design: Design,
}
impl Design {
    pub fn validate(&self, assets: &[Asset]) -> Result<(), String> {
        if !["file", "sine", "triangle", "square", "noise", "sweep"].contains(&self.source.as_str())
        {
            return Err("音源の種類が不正です".into());
        }
        model::range(self.frequency, 20., 16000., "周波数")?;
        model::range(self.end_frequency, 20., 16000., "終点周波数")?;
        model::range(self.duration_ms, 20., 60000., "音声の長さ")?;
        model::range(self.offset_ms, 0., 600000., "素材の開始位置")?;
        model::range(self.level_db, -60., 12., "音声レベル")?;
        model::range(self.azimuth, -180., 180., "方位")?;
        model::range(self.end_azimuth, -180., 180., "移動先")?;
        model::range(self.elevation, -40., 90., "仰角")?;
        model::range(self.distance_m, 0.3, 5., "音源距離")?;
        model::range(self.speed, -360., 360., "回転速度")?;
        if !["fixed", "orbit", "sweep", "path"].contains(&self.motion.as_str()) {
            return Err("音源の移動方法が不正です".into());
        }
        if self.keyframes.len() > 32 {
            return Err("経路は32点までです".into());
        }
        if self.motion == "path" || !self.keyframes.is_empty() {
            if self.keyframes.len() < 2
                || self.keyframes[0].at != 0.
                || self.keyframes.last().unwrap().at != 1.
            {
                return Err("経路には開始と終了の点が必要です".into());
            }
            for (i, key) in self.keyframes.iter().enumerate() {
                model::range(key.at, 0., 1., "通過点の時刻")?;
                model::range(key.azimuth, -180., 180., "通過点の方位")?;
                model::range(key.elevation, -40., 90., "通過点の仰角")?;
                model::range(key.distance_m, 0.3, 5., "通過点の距離")?;
                if i > 0 && key.at - self.keyframes[i - 1].at < 0.000999999 {
                    return Err("通過点は再生時間の0.1%以上の間隔で配置してください".into());
                }
            }
        }
        if self.source == "file" {
            let a = assets
                .iter()
                .find(|a| Some(&a.id) == self.asset_id.as_ref())
                .ok_or("音声ファイルを選択してください")?;
            if model::ms_frame(self.offset_ms) + model::ms_frame(self.duration_ms) > a.frames {
                return Err("再生範囲が音声ファイルの終端を超えています".into());
            }
        }
        Ok(())
    }
}
/// Position at normalized clip time. Legacy sweep intentionally keeps its original angular interpolation.
fn position_at(d: &Design, progress: f64) -> Keyframe {
    let t = progress.clamp(0., 1.);
    let mut pose = Keyframe {
        at: t,
        azimuth: d.azimuth,
        elevation: d.elevation,
        distance_m: d.distance_m,
    };
    match d.motion.as_str() {
        "orbit" => pose.azimuth += d.speed * t * d.duration_ms / 1000.,
        "sweep" => pose.azimuth += (d.end_azimuth - d.azimuth) * t,
        "path" => {
            let next = d
                .keyframes
                .partition_point(|key| key.at < t)
                .max(1)
                .min(d.keyframes.len() - 1);
            let a = d.keyframes[next - 1];
            let b = d.keyframes[next];
            let blend = (t - a.at) / (b.at - a.at);
            let raw = b.azimuth - a.azimuth;
            let mut angle = (raw + 180.).rem_euclid(360.) - 180.;
            if angle == -180. && raw > 0. {
                angle = 180.;
            }
            pose.azimuth = a.azimuth + angle * blend;
            pose.elevation = a.elevation + (b.elevation - a.elevation) * blend;
            pose.distance_m = a.distance_m + (b.distance_m - a.distance_m) * blend;
        }
        _ => {}
    }
    pose.azimuth = (pose.azimuth + 180.).rem_euclid(360.) - 180.;
    pose
}
pub fn asset_path(dir: &Path, id: &str) -> Result<PathBuf, String> {
    if id.len() != 70
        || !id.starts_with("audio-")
        || !id[6..].bytes().all(|c| c.is_ascii_hexdigit())
    {
        return Err("音声素材IDが不正です".into());
    }
    Ok(dir.join("assets").join(format!("{id}.wav")))
}
pub fn validate_project(p: &Project) -> Result<(), String> {
    if p.audio_assets.len() > 64 || p.audio_clips.len() > 128 {
        return Err("音声素材またはクリップが多すぎます".into());
    }
    let mut ids = std::collections::HashSet::new();
    for a in &p.audio_assets {
        asset_path(Path::new("."), &a.id)?;
        if !ids.insert(&a.id)
            || a.name.trim().is_empty()
            || a.name.chars().count() > 200
            || a.frames == 0
            || a.frames > RATE as usize * 600
            || a.source_channels == 0
            || a.peaks.len() > 1000
            || a.peaks.iter().any(|v| !v.is_finite() || *v < 0.)
        {
            return Err("音声素材の情報が不正です".into());
        }
    }
    p.audio_draft.validate(&p.audio_assets)?;
    ids.clear();
    for c in &p.audio_clips {
        if c.id.is_empty()
            || c.id.len() > 80
            || !ids.insert(&c.id)
            || c.name.trim().is_empty()
            || c.name.chars().count() > 200
        {
            return Err("音声クリップの情報が不正です".into());
        }
        c.design.validate(&p.audio_assets)?;
        model::range(c.start_ms, 0., p.duration_ms, "音声の配置位置")?;
        model::range(c.gain_db, -60., 24., "音声の補正")?;
        if model::ms_frame(c.start_ms) + model::ms_frame(c.design.duration_ms)
            > model::ms_frame(p.duration_ms)
        {
            return Err("音声がタイムラインの終端を超えています".into());
        }
    }
    Ok(())
}
pub fn import_file(path: &Path, dir: &Path) -> Result<Asset, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    if bytes.len() > 128 * 1024 * 1024 {
        return Err("音声ファイルは128 MB以内にしてください".into());
    }
    let id = format!("audio-{:x}", Sha256::digest(&bytes));
    let mss = MediaSourceStream::new(Box::new(std::io::Cursor::new(bytes)), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|x| x.to_str()) {
        hint.with_extension(ext);
    }
    let mut format = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| format!("音声を読み込めません: {e}"))?
        .format;
    let track = format.default_track().ok_or("音声トラックがありません")?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| e.to_string())?;
    let mut samples = Vec::new();
    let mut rate = 0;
    let mut channels = 0;
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(format!("音声の読み込みに失敗しました: {e}")),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = decoder
            .decode(&packet)
            .map_err(|e| format!("音声のデコードに失敗しました: {e}"))?;
        let spec = *decoded.spec();
        let count = spec.channels.count();
        if rate == 0 {
            rate = spec.rate;
            channels = count;
        }
        if rate != spec.rate || channels != count || channels == 0 {
            return Err("途中で音声形式が変化するファイルには未対応です".into());
        }
        let mut buffer = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        buffer.copy_interleaved_ref(decoded);
        for f in buffer.samples().chunks_exact(count) {
            let l = f[0];
            let r = if count > 1 { f[1] } else { l };
            if !l.is_finite() || !r.is_finite() {
                return Err("音声に不正な数値があります".into());
            }
            samples.extend([l, r]);
        }
        if samples.len() / 2 > rate as usize * 600 {
            return Err("音声は10分以内にしてください".into());
        }
    }
    if samples.is_empty() || rate == 0 {
        return Err("音声データが空です".into());
    }
    let samples = resample_stereo(&samples, rate);
    let frames = samples.len() / 2;
    let mut peaks = vec![];
    let step = frames.div_ceil(800).max(1);
    for block in samples.chunks(step * 2) {
        peaks.push(model::peak(block));
    }
    let output = asset_path(dir, &id)?;
    std::fs::create_dir_all(output.parent().unwrap()).map_err(|e| e.to_string())?;
    if !output.exists() {
        let temp = output.with_extension("tmp");
        write_float_wav(&temp, &samples)?;
        std::fs::rename(temp, &output).map_err(|e| e.to_string())?;
    }
    Ok(Asset {
        id,
        name: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .chars()
            .take(200)
            .collect(),
        frames,
        source_channels: channels,
        peaks,
    })
}
fn resample_stereo(input: &[f32], rate: u32) -> Vec<f32> {
    if rate == RATE {
        return input.to_vec();
    }
    let frames = input.len() / 2;
    let n = (frames as f64 * RATE as f64 / rate as f64).round() as usize;
    let mut out = vec![0.; n * 2];
    for (i, f) in out.chunks_exact_mut(2).enumerate() {
        let t = i as f64 * rate as f64 / RATE as f64;
        let a = (t as usize).min(frames - 1);
        let b = (a + 1).min(frames - 1);
        let v = (t - a as f64) as f32;
        for ch in 0..2 {
            f[ch] = input[2 * a + ch] * (1. - v) + input[2 * b + ch] * v;
        }
    }
    out
}
fn write_float_wav(path: &Path, samples: &[f32]) -> Result<(), String> {
    let mut w = hound::WavWriter::create(
        path,
        hound::WavSpec {
            channels: 2,
            sample_rate: RATE,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        },
    )
    .map_err(|e| e.to_string())?;
    for v in samples {
        w.write_sample(*v).map_err(|e| e.to_string())?
    }
    w.finalize().map_err(|e| e.to_string())
}
fn read_asset(dir: &Path, a: &Asset) -> Result<Vec<f32>, String> {
    let path = asset_path(dir, &a.id)?;
    let mut reader = hound::WavReader::open(path).map_err(|_| {
        format!(
            "素材「{}」が見つかりません。保存時のassetsフォルダーも必要です",
            a.name
        )
    })?;
    let spec = reader.spec();
    if spec.channels != 2
        || spec.sample_rate != RATE
        || spec.sample_format != hound::SampleFormat::Float
        || reader.duration() as usize != a.frames
    {
        return Err("保存された音声素材の形式が不正です".into());
    }
    reader
        .samples::<f32>()
        .map(|s| s.map_err(|e| e.to_string()))
        .collect()
}

struct Hrir {
    az: f32,
    el: f32,
    ir: Vec<[f32; 2]>,
}
fn bank() -> &'static Vec<Hrir> {
    static BANK: OnceLock<Vec<Hrir>> = OnceLock::new();
    BANK.get_or_init(|| {
        let bytes = include_bytes!("../assets/kemar.bin");
        let u32_at = |i| u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap()) as usize;
        let count = u32_at(8);
        let taps = u32_at(12);
        let mut at = 16;
        let mut bank = vec![];
        for _ in 0..count {
            let mut f = || {
                let v = f32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
                at += 4;
                v
            };
            let az = f();
            let el = f();
            let ir = (0..taps).map(|_| [f(), f()]).collect();
            bank.push(Hrir { az, el, ir });
        }
        bank
    })
}
fn hrtf(az: f64, el: f64) -> Vec<[f32; 2]> {
    let az = ((az + 180.).rem_euclid(360.) - 180.) as f32;
    let mirrored = az < 0.;
    let a = az.abs();
    let e = el as f32;
    let bank = bank();
    let low = bank
        .iter()
        .filter(|h| h.el <= e)
        .map(|h| h.el)
        .fold(-40., f32::max);
    let high = bank
        .iter()
        .filter(|h| h.el >= e)
        .map(|h| h.el)
        .fold(90., f32::min);
    let row = |e: f32| {
        let lo = bank
            .iter()
            .filter(|h| h.el == e && h.az <= a)
            .max_by(|x, y| x.az.total_cmp(&y.az))
            .unwrap();
        let hi = bank
            .iter()
            .filter(|h| h.el == e && h.az >= a)
            .min_by(|x, y| x.az.total_cmp(&y.az))
            .unwrap_or(lo);
        let t = if hi.az == lo.az {
            0.
        } else {
            (a - lo.az) / (hi.az - lo.az)
        };
        lo.ir
            .iter()
            .zip(&hi.ir)
            .map(|(x, y)| [x[0] * (1. - t) + y[0] * t, x[1] * (1. - t) + y[1] * t])
            .collect::<Vec<_>>()
    };
    let l = row(low);
    let h = row(high);
    let t = if low == high {
        0.
    } else {
        (e - low) / (high - low)
    };
    l.iter()
        .zip(&h)
        .map(|(x, y)| {
            let pair = [x[0] * (1. - t) + y[0] * t, x[1] * (1. - t) + y[1] * t];
            if mirrored {
                [pair[1], pair[0]]
            } else {
                pair
            }
        })
        .collect()
}
pub fn render_design(d: &Design, assets: &[Asset], dir: &Path) -> Result<Vec<f32>, String> {
    d.validate(assets)?;
    let frames = model::ms_frame(d.duration_ms);
    let mut source = vec![0.; frames * 2];
    if d.source == "file" {
        let a = assets
            .iter()
            .find(|a| Some(&a.id) == d.asset_id.as_ref())
            .unwrap();
        let data = read_asset(dir, a)?;
        let offset = model::ms_frame(d.offset_ms);
        source.copy_from_slice(&data[offset * 2..(offset + frames) * 2]);
    } else {
        let mut seed = 0x12345678u32;
        let mut phase = 0.;
        for (i, f) in source.chunks_exact_mut(2).enumerate() {
            let freq = if d.source == "sweep" {
                d.frequency * (d.end_frequency / d.frequency).powf(i as f64 / frames as f64)
            } else {
                d.frequency
            };
            let value = match d.source.as_str() {
                "noise" => {
                    seed ^= seed << 13;
                    seed ^= seed >> 17;
                    seed ^= seed << 5;
                    seed as f64 / u32::MAX as f64 * 2. - 1.
                }
                "triangle" => 2. / std::f64::consts::PI * (phase as f64).sin().asin(),
                "square" => {
                    if (phase as f64).sin() >= 0. {
                        1.
                    } else {
                        -1.
                    }
                }
                _ => (phase as f64).sin(),
            };
            phase = (phase + 2. * std::f64::consts::PI * freq / RATE as f64)
                .rem_euclid(2. * std::f64::consts::PI);
            f[0] = value as f32;
            f[1] = value as f32;
        }
    }
    let fade = 240usize.min(frames / 2);
    for (i, f) in source.chunks_exact_mut(2).enumerate() {
        let env = (i.min(frames - 1 - i) as f32 / fade as f32).min(1.);
        f[0] *= env;
        f[1] *= env;
    }
    let gain = model::db_gain(d.level_db) as f32;
    if !d.spatial {
        for v in &mut source {
            *v *= gain;
        }
        model::check_peak(&source, 1.)?;
        return Ok(source);
    }
    let mono: Vec<f32> = source
        .chunks_exact(2)
        .map(|f| (f[0] + f[1]) * 0.5)
        .collect();
    let mut output = vec![0.; frames * 2];
    let mut pose = position_at(d, 0.);
    let mut filter = hrtf(pose.azimuth, pose.elevation);
    let block = 384;
    let mut start = 0;
    while start < frames {
        let mut end = (start + block).min(frames);
        if d.motion == "path" {
            if let Some(boundary) = d
                .keyframes
                .iter()
                .map(|k| (k.at * frames as f64).ceil() as usize)
                .find(|n| *n > start)
            {
                end = end.min(boundary);
            }
        }
        let next_pose = position_at(d, end as f64 / frames as f64);
        let next = hrtf(next_pose.azimuth, next_pose.elevation);
        for i in start..end {
            let blend = (i - start) as f32 / (end - start) as f32;
            let distance =
                pose.distance_m as f32 * (1. - blend) + next_pose.distance_m as f32 * blend;
            let atten = gain / distance.max(1.);
            let mut frame = [0.; 2];
            for k in 0..filter.len().min(i + 1) {
                let v = mono[i - k];
                frame[0] += v * (filter[k][0] * (1. - blend) + next[k][0] * blend);
                frame[1] += v * (filter[k][1] * (1. - blend) + next[k][1] * blend);
            }
            output[i * 2] = frame[0] * atten;
            output[i * 2 + 1] = frame[1] * atten;
        }
        filter = next;
        pose = next_pose;
        start = end;
    }
    model::check_peak(&output, 1.)?;
    Ok(output)
}
pub fn route_headphones(stereo: &[f32], routing: [usize; 4]) -> Vec<f32> {
    let mut output = vec![0.; stereo.len() * 2];
    for (i, f) in stereo.chunks_exact(2).enumerate() {
        output[4 * i + routing[0] - 1] = f[0];
        output[4 * i + routing[1] - 1] = f[1];
    }
    output
}
pub fn render_project(p: &Project, dir: &Path, target: &str) -> Result<Vec<f32>, String> {
    p.validate()?;
    if !["all", "audio", "air"].contains(&target) {
        return Err("再生対象が不正です".into());
    }
    let mut output = if target == "audio" {
        vec![0.; model::ms_frame(p.duration_ms) * 4]
    } else {
        p.render()?
    };
    if target != "air" {
        for c in &p.audio_clips {
            let stereo = render_design(&c.design, &p.audio_assets, dir)?;
            let gain = model::db_gain(c.gain_db) as f32;
            let start = model::ms_frame(c.start_ms);
            for (i, f) in stereo.chunks_exact(2).enumerate() {
                output[(start + i) * 4 + p.settings.routing[0] - 1] += f[0] * gain;
                output[(start + i) * 4 + p.settings.routing[1] - 1] += f[1] * gain;
            }
        }
    }
    model::check_peak(&output, 1.)?;
    Ok(output)
}
pub fn copy_assets(p: &Project, from: &Path, to: &Path) -> Result<(), String> {
    for a in &p.audio_assets {
        let src = asset_path(from, &a.id)?;
        let dst = asset_path(to, &a.id)?;
        if src == dst {
            continue;
        }
        if !src.exists() {
            return Err(format!("素材「{}」が見つかりません", a.name));
        }
        std::fs::create_dir_all(dst.parent().unwrap()).map_err(|e| e.to_string())?;
        if !dst.exists() {
            std::fs::copy(src, dst).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
#[cfg(test)]
#[path = "auditory_tests.rs"]
mod tests;
