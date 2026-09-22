use serde::{Deserialize, Serialize};

pub const RATE: u32 = 48_000;
pub const MAX_WAVE_MS: f64 = 80.0;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Part {
    pub kind: String,
    pub start_ms: f64,
    pub amplitude: f64,
    pub frequency: f64,
    pub cycles: f64,
    pub push_ms: f64,
    pub return_ms: f64,
    pub width_ms: f64,
    pub polarity: i8,
}
impl Part {
    pub fn new(kind: &str) -> Self {
        Self {
            kind: kind.into(),
            start_ms: 0.,
            amplitude: 100.,
            frequency: 40.,
            cycles: 2.,
            push_ms: 12.,
            return_ms: 48.,
            width_ms: if kind == "bipolar" { 100. } else { 20. },
            polarity: 1,
        }
    }
    pub fn duration(&self) -> f64 {
        match self.kind.as_str() {
            "push" => self.push_ms + self.return_ms,
            "pulse" => self.width_ms,
            "bipolar" => 2. * self.width_ms,
            _ => self.cycles * 1000. / self.frequency,
        }
    }
    fn validate(&self) -> Result<(), String> {
        if !["push", "sine", "triangle", "pulse", "bipolar"].contains(&self.kind.as_str()) {
            return Err("不明な部品です".into());
        }
        let limit = if self.kind == "bipolar" {
            200.
        } else {
            MAX_WAVE_MS
        };
        range(self.start_ms, 0., limit, "配置位置")?;
        range(self.amplitude, 0., 100., "部品の振幅")?;
        range(self.frequency, 1., 200., "周波数")?;
        range(self.cycles, 0.1, 16., "周期数")?;
        range(self.push_ms, 1., 79., "押し出し")?;
        range(self.return_ms, 1., 79., "戻り")?;
        range(
            self.width_ms,
            1.,
            if self.kind == "bipolar" { 100. } else { 80. },
            "パルス幅",
        )?;
        if self.polarity != 1 && self.polarity != -1 {
            return Err("極性は正または反転です".into());
        }
        if self.start_ms + self.duration() > limit + 1e-8 {
            return Err(format!(
                "配置位置と波形の長さを{limit} ms以内にしてください"
            ));
        }
        Ok(())
    }
    pub fn sample(&self, time_ms: f64) -> f64 {
        use std::f64::consts::PI;
        let t = time_ms - self.start_ms;
        let len = self.duration();
        if t < 0. || t >= len {
            return 0.;
        }
        let v = match self.kind.as_str() {
            "push" => {
                if t < self.push_ms {
                    (PI * t / self.push_ms).sin().powi(2)
                } else {
                    -self.push_ms / self.return_ms
                        * (PI * (t - self.push_ms) / self.return_ms).sin().powi(2)
                }
            }
            "pulse" => (PI * t / self.width_ms).sin().powi(2),
            // SHITARA Fig. 5(a), b=1: negative precharge followed by positive drive.
            "bipolar" => {
                if t < self.width_ms {
                    -1.
                } else {
                    1.
                }
            }
            _ => {
                let phase = 2. * PI * self.frequency * t / 1000.;
                let v = if self.kind == "triangle" {
                    2. / PI * phase.sin().asin()
                } else {
                    phase.sin()
                };
                let edge = 2.0_f64.min(len / 4.);
                let env = if t < edge {
                    0.5 - 0.5 * (PI * t / edge).cos()
                } else if t > len - edge {
                    0.5 - 0.5 * (PI * (len - t) / edge).cos()
                } else {
                    1.
                };
                v * env
            }
        };
        v * self.amplitude / 100. * self.polarity as f64
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Wave {
    pub id: String,
    pub name: String,
    pub parts: Vec<Part>,
    pub level_db: f64,
}
impl Wave {
    pub fn duration(&self) -> f64 {
        self.parts
            .iter()
            .map(|p| p.start_ms + p.duration())
            .fold(0., f64::max)
    }
    pub fn validate(&self) -> Result<(), String> {
        text(&self.id, 80, "波形ID")?;
        text(&self.name, 120, "波形名")?;
        range(self.level_db, -60., 0., "出力レベル")?;
        if self.parts.is_empty() || self.parts.len() > 16 {
            return Err("部品は1〜16個にしてください".into());
        }
        for p in &self.parts {
            p.validate()?;
        }
        Ok(())
    }
    pub fn mono(&self, apply_level: bool) -> Result<Vec<f32>, String> {
        self.validate()?;
        let gain = if apply_level {
            db_gain(self.level_db)
        } else {
            1.
        };
        let frames = ms_frame(self.duration());
        Ok((0..frames)
            .map(|i| {
                (self
                    .parts
                    .iter()
                    .map(|p| p.sample(i as f64 * 1000. / RATE as f64))
                    .sum::<f64>()
                    * gain) as f32
            })
            .collect())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Clip {
    pub id: String,
    pub wave_id: String,
    pub lane: String,
    pub start_ms: f64,
    pub gain_db: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub device_id: Option<String>,
    pub routing: [usize; 4],
    pub distance_cm: [f64; 2],
    pub preview_lane: String,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            device_id: None,
            routing: [1, 2, 3, 4],
            distance_cm: [30., 30.],
            preview_lane: "left".into(),
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        let mut sorted = self.routing;
        sorted.sort();
        if sorted != [1, 2, 3, 4] {
            return Err("4つの用途を1〜4 chに重複なく割り当ててください".into());
        }
        if !["left", "right"].contains(&self.preview_lane.as_str()) {
            return Err("プレビュー先が不正です".into());
        }
        for d in self.distance_cm {
            range(d, 1., 300., "距離")?;
        }
        if self.device_id.as_ref().is_some_and(|s| s.len() > 2048) {
            return Err("デバイスIDが長すぎます".into());
        }
        Ok(())
    }
    pub fn channel(&self, lane: &str) -> Result<usize, String> {
        match lane {
            "left" => Ok(self.routing[2] - 1),
            "right" => Ok(self.routing[3] - 1),
            _ => Err("左右の指定が不正です".into()),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Project {
    pub version: u32,
    pub draft: Wave,
    pub waves: Vec<Wave>,
    pub clips: Vec<Clip>,
    pub duration_ms: f64,
    pub settings: Settings,
    #[serde(default)]
    pub audio_assets: Vec<crate::auditory::Asset>,
    #[serde(default)]
    pub audio_clips: Vec<crate::auditory::AudioClip>,
    #[serde(default)]
    pub audio_draft: crate::auditory::Design,
}
// Stable IDs allow new presets to be added without replacing saved work.
pub fn default_waves() -> Vec<Wave> {
    let wave = |id: &str, name: &str, part: Part| Wave {
        id: id.into(),
        name: name.into(),
        parts: vec![part],
        level_db: -18.,
    };
    let push = |a: f64, b: f64| Part {
        push_ms: a,
        return_ms: b,
        ..Part::new("push")
    };
    let pulse = |width: f64| Part {
        width_ms: width,
        ..Part::new("pulse")
    };
    let sine = |frequency: f64, cycles: f64| Part {
        frequency,
        cycles,
        ..Part::new("sine")
    };
    vec![
        wave("single", "単発パルス", Part::new("push")),
        wave("rounded", "丸めパルス", Part::new("pulse")),
        wave("triangle", "三角波 40 Hz", Part::new("triangle")),
        wave("sine40", "正弦波 40 Hz", Part::new("sine")),
        wave("preset-push-short", "短い押し出し", push(6., 24.)),
        wave("preset-push-long", "長い押し出し", push(16., 64.)),
        wave("preset-push-fast", "速い押し出し", push(4., 56.)),
        wave("preset-push-even", "押し出し・戻り同幅", push(20., 20.)),
        wave("preset-pulse-short", "短い丸めパルス", pulse(8.)),
        wave("preset-pulse-long", "長い丸めパルス", pulse(40.)),
        wave("preset-sine30", "正弦波 30 Hz", sine(30., 2.)),
        wave("preset-sine60", "正弦波 60 Hz", sine(60., 3.)),
        wave(
            "preset-shitara-b1",
            "SHITARA 矩形二相波 b=1（100+100 ms）",
            Part::new("bipolar"),
        ),
    ]
}
impl Default for Project {
    fn default() -> Self {
        let waves = default_waves();
        Self {
            version: 1,
            draft: Wave {
                id: "draft".into(),
                name: "パルス 01".into(),
                ..waves[0].clone()
            },
            waves,
            clips: vec![],
            duration_ms: 2000.,
            settings: Settings::default(),
            audio_assets: vec![],
            audio_clips: vec![],
            audio_draft: crate::auditory::Design::default(),
        }
    }
}
impl Project {
    pub fn ensure_presets(&mut self) {
        for wave in default_waves() {
            if !self.waves.iter().any(|old| old.id == wave.id) {
                self.waves.push(wave);
            }
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("未対応のプロジェクト形式です".into());
        }
        crate::auditory::validate_project(self)?;
        self.draft.validate()?;
        self.settings.validate()?;
        range(self.duration_ms, 80., 60_000., "シーケンスの長さ")?;
        if self.waves.len() > 256 + default_waves().len() || self.clips.len() > 512 {
            return Err("波形またはクリップが多すぎます".into());
        }
        let mut ids = std::collections::HashSet::new();
        for w in &self.waves {
            w.validate()?;
            if !ids.insert(w.id.as_str()) {
                return Err("波形IDが重複しています".into());
            }
        }
        let mut ids = std::collections::HashSet::new();
        for c in &self.clips {
            text(&c.id, 80, "クリップID")?;
            if !ids.insert(c.id.as_str()) {
                return Err("クリップIDが重複しています".into());
            }
            self.settings.channel(&c.lane)?;
            range(c.start_ms, 0., self.duration_ms, "開始位置")?;
            range(c.gain_db, -60., 24., "レベル補正")?;
            let w = self
                .waves
                .iter()
                .find(|w| w.id == c.wave_id)
                .ok_or("参照する波形がありません")?;
            if ms_frame(c.start_ms) + ms_frame(w.duration()) > ms_frame(self.duration_ms) {
                return Err("クリップがシーケンスの終端を超えています".into());
            }
        }
        Ok(())
    }
    pub fn render(&self) -> Result<Vec<f32>, String> {
        self.validate()?;
        let mut buffer = vec![0.; ms_frame(self.duration_ms) * 4];
        for c in &self.clips {
            let w = self.waves.iter().find(|w| w.id == c.wave_id).unwrap();
            let mono = w.mono(true)?;
            let ch = self.settings.channel(&c.lane)?;
            let start = ms_frame(c.start_ms);
            let gain = db_gain(c.gain_db) as f32;
            for (i, v) in mono.iter().enumerate() {
                buffer[(start + i) * 4 + ch] += v * gain;
            }
        }
        check_peak(&buffer, 1.)?;
        Ok(buffer)
    }
}
pub fn ms_frame(ms: f64) -> usize {
    (ms * RATE as f64 / 1000.).round() as usize
}
pub fn db_gain(db: f64) -> f64 {
    10_f64.powf(db / 20.)
}
pub fn peak(samples: &[f32]) -> f32 {
    samples.iter().map(|s| s.abs()).fold(0., f32::max)
}
pub fn check_peak(samples: &[f32], gain: f32) -> Result<(), String> {
    if samples.iter().any(|v| !v.is_finite()) || peak(samples) * gain > 1.000001 {
        Err(
            "合成波形が0 dBFSを超えています。出力レベル・レベル補正・部品の振幅を下げてください"
                .into(),
        )
    } else {
        Ok(())
    }
}
pub fn range(v: f64, min: f64, max: f64, name: &str) -> Result<(), String> {
    if !v.is_finite() || v < min || v > max {
        Err(format!("{name}は{min}〜{max}の範囲です"))
    } else {
        Ok(())
    }
}
fn text(s: &str, max: usize, name: &str) -> Result<(), String> {
    if s.trim().is_empty() || s.chars().count() > max || s.contains('\0') {
        Err(format!("{name}が不正です"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sample_count_and_80ms_boundary() {
        let mut w = Project::default().draft;
        assert_eq!(w.mono(false).unwrap().len(), 2880);
        w.parts[0].start_ms = 20.;
        assert_eq!(w.mono(false).unwrap().len(), 3840);
        w.parts[0].start_ms = 20.1;
        assert!(w.validate().is_err());
    }
    #[test]
    fn shitara_b1_has_two_equal_constant_phases_and_exact_edges() {
        let wave = default_waves()
            .into_iter()
            .find(|w| w.id == "preset-shitara-b1")
            .unwrap();
        let samples = wave.mono(false).unwrap();
        assert_eq!(samples.len(), 9600);
        assert!(samples[..4800].iter().all(|x| *x == -1.));
        assert!(samples[4800..].iter().all(|x| *x == 1.));
        assert_eq!(samples.iter().sum::<f32>(), 0.);
        assert_eq!(wave.parts[0].sample(200.), 0.);
        let mut shifted = wave.clone();
        shifted.parts[0].start_ms = 0.1;
        assert!(shifted.validate().is_err());
    }
    #[test]
    fn push_return_displaces_and_balances() {
        let w = Project::default().draft;
        let v = w.mono(false).unwrap();
        assert!((v[288] - 1.).abs() < 1e-6);
        assert!((v[1728] + 0.25).abs() < 1e-6);
        assert!(v.iter().sum::<f32>().abs() < 0.001);
    }
    #[test]
    fn gain_is_applied_once() {
        let w = Project::default().draft;
        assert!((peak(&w.mono(true).unwrap()) - 0.12589254).abs() < 1e-6);
    }
    #[test]
    fn routing_and_sample_offset() {
        let mut p = Project::default();
        p.settings.routing = [3, 4, 1, 2];
        p.clips.push(Clip {
            id: "c".into(),
            wave_id: "single".into(),
            lane: "right".into(),
            start_ms: 100.,
            gain_db: 0.,
        });
        let b = p.render().unwrap();
        assert!(b[..4800 * 4].iter().all(|s| *s == 0.));
        assert!(b
            .chunks_exact(4)
            .all(|f| f[0] == 0. && f[2] == 0. && f[3] == 0.));
        assert!((b[(4800 + 288) * 4 + 1] - 0.12589254).abs() < 1e-6);
    }
    #[test]
    fn overlap_is_rejected_without_normalizing() {
        let mut p = Project::default();
        p.waves[0].level_db = 0.;
        for i in 0..2 {
            p.clips.push(Clip {
                id: i.to_string(),
                wave_id: "single".into(),
                lane: "left".into(),
                start_ms: 0.,
                gain_db: 0.,
            })
        }
        assert!(p.render().is_err());
    }
    #[test]
    fn invalid_routing_and_missing_wave_are_rejected() {
        let mut p = Project::default();
        p.settings.routing = [1, 2, 3, 3];
        assert!(p.validate().is_err());
        p.settings = Settings::default();
        p.clips.push(Clip {
            id: "a".into(),
            wave_id: "missing".into(),
            lane: "left".into(),
            start_ms: 0.,
            gain_db: 0.,
        });
        assert!(p.validate().is_err());
    }
    #[test]
    fn imported_schema_roundtrip() {
        let p = Project::default();
        let encoded = serde_json::to_vec(&p).unwrap();
        let q: Project = serde_json::from_slice(&encoded).unwrap();
        q.validate().unwrap();
        assert_eq!(p.draft, q.draft);
    }
}

#[cfg(test)]
mod preset_gain_tests {
    use super::*;
    #[test]
    fn presets_are_distinct_and_fit_the_editor() {
        let waves = default_waves();
        assert_eq!(waves.len(), 13);
        let mut ids = std::collections::HashSet::new();
        let mut rendered = vec![];
        for w in waves {
            w.validate().unwrap();
            assert!(ids.insert(w.id.clone()));
            assert!(
                w.duration()
                    <= if w.parts[0].kind == "bipolar" {
                        200.
                    } else {
                        80.
                    }
            );
            let samples = w.mono(true).unwrap();
            check_peak(&samples, 1.).unwrap();
            assert!(peak(&samples) > 0.);
            assert!(!rendered.contains(&samples));
            rendered.push(samples);
        }
    }
    #[test]
    fn adding_presets_preserves_saved_work_and_is_idempotent() {
        let mut p = Project::default();
        p.waves.truncate(4);
        p.waves[0].name = "既存の編集済み波形".into();
        p.waves[0].level_db = -12.;
        p.draft.name = "作業中".into();
        p.settings.routing = [3, 4, 1, 2];
        let custom = Wave {
            id: "user-test".into(),
            name: "保存波形".into(),
            ..p.waves[0].clone()
        };
        p.waves.push(custom.clone());
        p.clips.push(Clip {
            id: "clip".into(),
            wave_id: custom.id.clone(),
            lane: "right".into(),
            start_ms: 100.,
            gain_db: 6.,
        });
        let draft = p.draft.clone();
        p.ensure_presets();
        p.ensure_presets();
        p.validate().unwrap();
        assert_eq!(p.waves.len(), default_waves().len() + 1);
        assert_eq!(p.waves[0].name, "既存の編集済み波形");
        assert_eq!(p.waves[4], custom);
        assert_eq!(p.draft, draft);
        assert_eq!(p.clips[0].gain_db, 6.);
        assert_eq!(p.settings.routing, [3, 4, 1, 2]);
    }
    #[test]
    fn full_legacy_library_can_receive_presets() {
        let mut p = Project::default();
        p.waves = (0..256)
            .map(|i| Wave {
                id: format!("user-{i}"),
                ..p.draft.clone()
            })
            .collect();
        p.ensure_presets();
        p.validate().unwrap();
        assert_eq!(p.waves.len(), 256 + default_waves().len());
    }
    #[test]
    fn positive_gain_boosts_render_and_survives_save() {
        let mut p = Project::default();
        p.clips.push(Clip {
            id: "c".into(),
            wave_id: "single".into(),
            lane: "left".into(),
            start_ms: 0.,
            gain_db: 12.,
        });
        let encoded = serde_json::to_vec(&p).unwrap();
        let saved: Project = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(saved.clips[0].gain_db, 12.);
        assert!((peak(&saved.render().unwrap()) - db_gain(-6.) as f32).abs() < 1e-6);
        p.waves[0].level_db = -30.;
        p.clips[0].gain_db = 24.;
        assert!((peak(&p.render().unwrap()) - db_gain(-6.) as f32).abs() < 1e-6);
        p.clips[0].gain_db = 24.5;
        assert!(p.validate().is_err());
    }
    #[test]
    fn positive_gain_still_checks_output_peak() {
        let mut p = Project::default();
        p.clips.push(Clip {
            id: "c".into(),
            wave_id: "single".into(),
            lane: "left".into(),
            start_ms: 0.,
            gain_db: 18.,
        });
        assert!((peak(&p.render().unwrap()) - 1.).abs() < 1e-6);
        p.clips[0].gain_db = 24.;
        p.validate().unwrap();
        assert!(p.render().is_err());
    }
}
