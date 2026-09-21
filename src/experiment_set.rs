//! Versioned, deterministic experiment plans and baked stimulus bundles.
use crate::{auditory, model::Project, unity_export};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Condition {
    pub id: String,
    pub modality: String,
    pub audio_side: String,
    pub air_side: String,
    /// Positive means air starts AFTER audio, in baked sample time.
    pub onset_diff_ms: i32,
    pub gain_db: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Plan {
    pub template: String,
    pub seed: u32,
    pub repetitions: u32,
    pub blocks: u32,
    pub participants: Vec<String>,
    pub primary: String,
    pub secondary: Vec<String>,
    pub conditions: Vec<Condition>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Trial {
    pub trial_id: String,
    pub participant_id: String,
    pub block: u32,
    pub repetition: u32,
    pub condition_id: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Manifest {
    pub schema_version: u32,
    pub generator_version: String,
    pub project_hash: String,
    pub hrtf_mode: String,
    pub device_info: Value,
    pub media_hashes: std::collections::BTreeMap<String, String>,
    pub project: Project,
    pub plan: Plan,
    pub stimuli: Vec<Value>,
    pub trials: Vec<Trial>,
}
pub fn hash<T: Serialize>(value: &T) -> Result<String, String> {
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(value).map_err(|e| e.to_string())?)
    ))
}
fn identifier(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 40
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_".contains(&c))
}
pub fn defaults(template: &str) -> Result<Vec<Condition>, String> {
    let mut rows = vec![];
    for side in ["left", "right"] {
        let mut add = |modality: &str, air: &str, offset, gain| {
            rows.push(Condition {
                id: format!("c{:02}", rows.len() + 1),
                modality: modality.into(),
                audio_side: if modality == "air" { "none" } else { side }.into(),
                air_side: if modality == "audio" { "none" } else { air }.into(),
                onset_diff_ms: offset,
                gain_db: gain,
            })
        };
        match template {
            "congruency" => {
                add("combined", side, 0, 0.);
                add(
                    "combined",
                    if side == "left" { "right" } else { "left" },
                    0,
                    0.,
                );
            }
            "onset" => {
                for offset in [-100, 0, 100] {
                    add("combined", side, offset, 0.);
                }
            }
            "modality" => {
                for modality in ["audio", "air", "combined"] {
                    add(modality, side, 0, 0.);
                }
            }
            "intensity" => {
                for gain in [-6., -3., 0.] {
                    add("combined", side, 0, gain);
                }
            }
            _ => return Err("実験テンプレートが不正です".into()),
        }
    }
    Ok(rows)
}
fn validate_plan(plan: &Plan) -> Result<(), String> {
    defaults(&plan.template)?;
    if !(1..=8).contains(&plan.repetitions)
        || !(1..=16).contains(&plan.blocks)
        || plan.participants.is_empty()
        || plan.participants.len() > 64
        || plan.conditions.is_empty()
        || plan.conditions.len() > 32
        || plan.participants.len()
            * plan.conditions.len()
            * plan.repetitions as usize
            * plan.blocks as usize
            > 32768
    {
        return Err(
            "参加者1〜64、条件1〜32、反復1〜8、ブロック1〜16、総試行32768以内にしてください".into(),
        );
    }
    let mut ids = HashSet::new();
    for id in &plan.participants {
        if !identifier(id) || !ids.insert(id.to_ascii_lowercase()) {
            return Err("匿名参加者IDは重複のない40文字以内の英数字・ハイフン・下線です".into());
        }
    }
    ids.clear();
    for c in &plan.conditions {
        if c.id.to_ascii_lowercase().starts_with("calibration-")
            || !identifier(&c.id)
            || !ids.insert(c.id.to_ascii_lowercase())
            || !["audio", "air", "combined"].contains(&c.modality.as_str())
            || !["left", "right", "none"].contains(&c.audio_side.as_str())
            || !["left", "right", "none"].contains(&c.air_side.as_str())
            || (c.audio_side == "none") != (c.modality == "air")
            || (c.air_side == "none") != (c.modality == "audio")
            || ![-100, 0, 100].contains(&c.onset_diff_ms)
            || (c.modality != "combined" && c.onset_diff_ms != 0)
            || !c.gain_db.is_finite()
            || !(-18.0..=0.0).contains(&c.gain_db)
        {
            return Err(
                "因子表のID、刺激、左右、開始差、補正（−18〜0 dB）を確認してください".into(),
            );
        }
    }
    let mut measures = HashSet::new();
    if !["direction", "reactionTime"].contains(&plan.primary.as_str())
        || plan.secondary.len() > 3
        || plan.secondary.iter().any(|s| {
            !["strength", "naturalness", "comfort"].contains(&s.as_str()) || !measures.insert(s)
        })
    {
        return Err("主指標は方向判断または反応時間、補助指標は強さ・自然さ・快適さです".into());
    }
    Ok(())
}
pub fn stimulus(base: &Project, c: &Condition) -> Result<Project, String> {
    base.validate()?;
    if base.audio_clips.len() != 1 || base.clips.len() != 1 {
        return Err(
            "実験セットの基準には音声1クリップと空気刺激1クリップを配置してください".into(),
        );
    }
    if base.audio_clips[0].design.duration_ms > 10000. {
        return Err("実験セットの音声は10秒以内にしてください".into());
    }
    let mut p = base.clone();
    // Start at a common reference 200 ms into every sequence. Negative offsets remain nonnegative.
    let audio = &mut p.audio_clips[0];
    audio.start_ms = 200.;
    audio.gain_db += c.gain_db;
    audio.design.spatial = true;
    audio.design.motion = "fixed".into();
    audio.design.azimuth = if c.audio_side == "left" { -90. } else { 90. };
    audio.design.elevation = 0.;
    audio.design.keyframes.clear();
    let air = &mut p.clips[0];
    air.start_ms = 200. + c.onset_diff_ms as f64;
    air.lane = if c.air_side == "none" {
        "left"
    } else {
        &c.air_side
    }
    .into();
    air.gain_db += c.gain_db;
    let wave = p
        .waves
        .iter()
        .find(|w| w.id == air.wave_id)
        .ok_or("基準波形がありません")?;
    // All conditions share duration, including single-modality controls.
    p.duration_ms = (300. + audio.design.duration_ms.max(wave.duration())) + 200.;
    if c.modality == "audio" {
        p.clips.clear();
    }
    if c.modality == "air" {
        p.audio_clips.clear();
    }
    p.validate()?;
    Ok(p)
}
pub fn calibration(
    base: &Project,
    source: &Path,
    index: usize,
) -> Result<(Project, Vec<f32>), String> {
    if index > 3 {
        return Err("確認する左右を指定してください".into());
    }
    let side = if index % 2 == 0 { "left" } else { "right" };
    let condition = Condition {
        id: "calibration".into(),
        modality: if index < 2 { "audio" } else { "air" }.into(),
        audio_side: side.into(),
        air_side: side.into(),
        onset_diff_ms: 0,
        gain_db: 0.,
    };
    let mut cue = stimulus(base, &condition)?;
    if index < 2 {
        cue.audio_clips[0].design.spatial = false;
    }
    let mut samples = auditory::render_project(&cue, source, "all")?;
    if index < 2 {
        let [left, right, _, _] = cue.settings.routing.map(|ch| ch - 1);
        for frame in samples.chunks_exact_mut(4) {
            let mono = (frame[left] + frame[right]) * 0.5;
            frame[left] = if index == 0 { mono } else { 0. };
            frame[right] = if index == 1 { mono } else { 0. };
        }
    }
    Ok((cue, samples))
}
fn order(plan: &Plan) -> Vec<Trial> {
    let mut trials = vec![];
    for participant in &plan.participants {
        for block in 1..=plan.blocks {
            let bytes = Sha256::digest(format!(
                "aircue-order-v1:{}:{}:{}",
                plan.seed, participant, block
            ));
            let mut state = u64::from_le_bytes(bytes[..8].try_into().unwrap());
            let mut rows = vec![];
            for repetition in 1..=plan.repetitions {
                for c in &plan.conditions {
                    rows.push((repetition, c.id.clone()));
                }
            }
            // Specified SplitMix64 + Fisher-Yates, independent of std or platform RNG changes.
            for i in (1..rows.len()).rev() {
                state = state.wrapping_add(0x9e3779b97f4a7c15);
                let mut n = state;
                n = (n ^ (n >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
                n = (n ^ (n >> 27)).wrapping_mul(0x94d049bb133111eb);
                n ^= n >> 31;
                rows.swap(i, (n % (i as u64 + 1)) as usize);
            }
            for (i, (repetition, condition_id)) in rows.into_iter().enumerate() {
                trials.push(Trial {
                    trial_id: format!("{participant}-b{block:02}-t{:04}", i + 1),
                    participant_id: participant.clone(),
                    block,
                    repetition,
                    condition_id,
                });
            }
        }
    }
    trials
}
fn media_hashes(
    base: &Project,
    source: &Path,
) -> Result<std::collections::BTreeMap<String, String>, String> {
    let mut hashes = std::collections::BTreeMap::new();
    for clip in &base.audio_clips {
        if clip.design.source == "file" {
            let id = clip
                .design
                .asset_id
                .as_ref()
                .ok_or("音声素材がありません")?;
            let bytes =
                std::fs::read(auditory::asset_path(source, id)?).map_err(|e| e.to_string())?;
            hashes.insert(id.clone(), format!("{:x}", Sha256::digest(bytes)));
        }
    }
    Ok(hashes)
}
pub fn generate(
    base: &Project,
    plan: Plan,
    device_info: Value,
    source: &Path,
) -> Result<Manifest, String> {
    let m = generate_metadata(base, plan, device_info, media_hashes(base, source)?)?;
    if serde_json::to_vec_pretty(&m)
        .map_err(|e| e.to_string())?
        .len()
        > 8 * 1024 * 1024
    {
        return Err("manifestが8 MBを超えます。参加者または試行数を減らしてください".into());
    }
    Ok(m)
}
fn generate_metadata(
    base: &Project,
    plan: Plan,
    device_info: Value,
    media_hashes: std::collections::BTreeMap<String, String>,
) -> Result<Manifest, String> {
    validate_plan(&plan)?;
    let mut stimuli = vec![];
    for c in &plan.conditions {
        let p = stimulus(base, c)?;
        stimuli.push(json!({"stimulusId":c.id,"revision":hash(&(&p,&media_hashes,env!("CARGO_PKG_VERSION")))?,"sequence":format!("Sequences/Sequence-{}/Sequence.aircueseq",c.id),"routing":p.settings.routing,"audioClips":p.audio_clips,"airClips":p.clips,"waveLevels":p.waves.iter().map(|w|json!({"id":w.id,"levelDb":w.level_db})).collect::<Vec<_>>()}));
    }
    Ok(Manifest {
        schema_version: 1,
        generator_version: env!("CARGO_PKG_VERSION").into(),
        project_hash: hash(base)?,
        hrtf_mode: "baked-stereo/mit-kemar-48000-140".into(),
        device_info,
        media_hashes,
        project: base.clone(),
        trials: order(&plan),
        plan,
        stimuli,
    })
}
pub fn validate(manifest: &Manifest) -> Result<(), String> {
    if manifest.schema_version != 1 || manifest.generator_version != env!("CARGO_PKG_VERSION") {
        return Err("この版に対応しない実験manifestです".into());
    }
    let ids: std::collections::BTreeSet<_> = manifest
        .project
        .audio_clips
        .iter()
        .filter(|c| c.design.source == "file")
        .filter_map(|c| c.design.asset_id.as_ref())
        .collect();
    if ids != manifest.media_hashes.keys().collect()
        || manifest
            .media_hashes
            .values()
            .any(|h| h.len() != 64 || !h.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return Err("音声素材のハッシュが不正です".into());
    }
    let regenerated = generate_metadata(
        &manifest.project,
        manifest.plan.clone(),
        manifest.device_info.clone(),
        manifest.media_hashes.clone(),
    )?;
    if serde_json::to_value(regenerated).map_err(|e| e.to_string())?
        != serde_json::to_value(manifest).map_err(|e| e.to_string())?
    {
        return Err(
            "実験manifestのハッシュ、刺激情報、試行順が一致しません。因子表から再生成してください"
                .into(),
        );
    }
    Ok(())
}
pub fn load(path: &Path) -> Result<Manifest, String> {
    if std::fs::metadata(path).map_err(|e| e.to_string())?.len() > 8 * 1024 * 1024 {
        return Err("manifestは8 MB以内です".into());
    }
    let m = serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    validate(&m)?;
    Ok(m)
}
pub fn export(
    m: &Manifest,
    current: &Project,
    source: &Path,
    destination: &Path,
) -> Result<PathBuf, String> {
    validate(m)?;
    if hash(current)? != m.project_hash || media_hashes(current, source)? != m.media_hashes {
        return Err("プロジェクトが変更されました。実験セットを再生成してください".into());
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let root = destination.join(format!("AirCue-Experiment-{stamp}"));
    std::fs::create_dir(&root).map_err(|e| e.to_string())?;
    let result = (|| {
        let mut checksums = vec![];
        for c in &m.plan.conditions {
            let p = stimulus(&m.project, c)?;
            let rendered = auditory::render_project(&p, source, "all")?;
            unity_export::write_package(&p, &rendered, &root, &c.id)?;
            let relative = format!("Sequences/Sequence-{}/routed-4ch.wav", c.id);
            checksums.push(json!({"stimulusId":c.id,"routedWavSha256":format!("{:x}",Sha256::digest(std::fs::read(root.join("AirCueUnity").join(relative)).map_err(|e|e.to_string())?))}));
        }
        for index in 0..4 {
            let (cue, rendered) = calibration(&m.project, source, index)?;
            unity_export::write_package(&cue, &rendered, &root, &format!("calibration-{index}"))?;
        }
        let package = root.join("AirCueUnity");
        std::fs::write(
            package.join("Experiment.aircueexp"),
            serde_json::to_vec_pretty(m).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        std::fs::write(
            package.join("checksums.json"),
            serde_json::to_vec_pretty(&checksums).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        Ok::<_, String>(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&root);
    }
    result.map(|_| root)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn altered_media_and_clipping_do_not_leave_partial_exports() {
        let root = std::env::temp_dir().join(format!("aircue-frozen-media-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("source.wav");
        crate::write_wav(&source, &vec![0.1; 9600], 2).unwrap();
        let asset = auditory::import_file(&source, &root).unwrap();
        let mut p = base();
        p.audio_clips[0].design.source = "file".into();
        p.audio_clips[0].design.asset_id = Some(asset.id.clone());
        p.audio_assets.push(asset.clone());
        let m = generate(&p, plan("modality"), Value::Null, &root).unwrap();
        validate(&m).unwrap();
        crate::write_wav(
            &auditory::asset_path(&root, &asset.id).unwrap(),
            &vec![0.2; 9600],
            2,
        )
        .unwrap();
        assert!(export(&m, &p, &root, &root).is_err());
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 2);
        p = base();
        p.clips[0].gain_db = 24.;
        p.waves[0].level_db = 0.;
        let m = generate(&p, plan("modality"), Value::Null, &root).unwrap();
        assert!(export(&m, &p, &root, &root).is_err());
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 2);
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn bundle_samples_match_signed_onsets_and_calibration_channels() {
        let root =
            std::env::temp_dir().join(format!("aircue-experiment-test-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let base = base();
        let manifest = generate(&base, plan("onset"), Value::Null, &root).unwrap();
        let output = export(&manifest, &base, &root, &root).unwrap();
        let package = output.join("AirCueUnity");
        validate(&load(&package.join("Experiment.aircueexp")).unwrap()).unwrap();
        for c in &manifest.plan.conditions {
            let samples: Vec<i32> = hound::WavReader::open(
                package.join(format!("Sequences/Sequence-{}/routed-4ch.wav", c.id)),
            )
            .unwrap()
            .samples::<i32>()
            .map(Result::unwrap)
            .collect();
            let channel = base.settings.routing[if c.air_side == "left" { 2 } else { 3 }] - 1;
            let start = (200 + c.onset_diff_ms) as usize * 48;
            assert!(samples.chunks_exact(4).take(start).all(|f| f[channel] == 0));
            assert!(samples
                .chunks_exact(4)
                .skip(start)
                .take(48)
                .any(|f| f[channel] != 0));
        }
        for i in 0..4 {
            let (_, samples) = calibration(&base, &root, i).unwrap();
            let channel = base.settings.routing[i] - 1;
            assert!(samples.chunks_exact(4).any(|f| f[channel] != 0.));
            assert!(samples
                .chunks_exact(4)
                .all(|f| f.iter().enumerate().all(|(i, v)| i == channel || *v == 0.)));
        }
        let mut edited = base.clone();
        edited.clips[0].gain_db -= 1.;
        assert!(export(&manifest, &edited, &root, &root).is_err());
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
        std::fs::remove_dir_all(&root).unwrap();
    }
    #[test]
    fn invalid_counts_levels_and_windows_case_collisions_are_rejected() {
        let mut p = plan("modality");
        p.conditions[1].id = p.conditions[0].id.to_uppercase();
        assert!(validate_plan(&p).is_err());
        p = plan("modality");
        p.conditions[0].id = "Calibration-0".into();
        assert!(validate_plan(&p).is_err());
        p = plan("modality");
        p.repetitions = 9;
        assert!(validate_plan(&p).is_err());
        p = plan("modality");
        p.conditions[0].gain_db = f64::NAN;
        assert!(validate_plan(&p).is_err());
        p = plan("modality");
        p.conditions[0].gain_db = 3.;
        assert!(validate_plan(&p).is_err());
        let mut b = base();
        b.clips.push(b.clips[0].clone());
        assert!(generate(&b, plan("onset"), Value::Null, Path::new(".")).is_err());
    }
    fn base() -> Project {
        serde_json::from_str(include_str!("../tests/unity-project.json")).unwrap()
    }
    fn plan(template: &str) -> Plan {
        Plan {
            template: template.into(),
            seed: 42,
            repetitions: 4,
            blocks: 2,
            participants: vec!["P001".into(), "P002".into()],
            primary: "direction".into(),
            secondary: vec![],
            conditions: defaults(template).unwrap(),
        }
    }
    #[test]
    fn deterministic_balanced_participant_blocks() {
        let p = plan("onset");
        let a = order(&p);
        assert_eq!(a, order(&p));
        assert_eq!(a.len(), 96);
        for id in &p.participants {
            for block in 1..=2 {
                for c in &p.conditions {
                    assert_eq!(
                        a.iter()
                            .filter(|t| &t.participant_id == id
                                && t.block == block
                                && t.condition_id == c.id)
                            .count(),
                        4
                    );
                }
            }
        }
        assert_ne!(
            a[..24].iter().map(|t| &t.condition_id).collect::<Vec<_>>(),
            a[48..72]
                .iter()
                .map(|t| &t.condition_id)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            a.iter().map(|t| &t.trial_id).collect::<HashSet<_>>().len(),
            a.len()
        );
    }
    #[test]
    fn timing_sides_levels_and_source_preservation() {
        let p = base();
        let before = hash(&p).unwrap();
        for template in ["onset", "congruency", "modality", "intensity"] {
            for c in defaults(template).unwrap() {
                let s = stimulus(&p, &c).unwrap();
                assert_eq!(s.settings.routing, p.settings.routing);
                if c.modality != "air" {
                    assert_eq!(s.audio_clips[0].start_ms, 200.);
                    assert_eq!(
                        s.audio_clips[0].design.azimuth,
                        if c.audio_side == "left" { -90. } else { 90. }
                    );
                }
                if c.modality != "audio" {
                    assert_eq!(s.clips[0].start_ms, 200. + c.onset_diff_ms as f64);
                    assert_eq!(s.clips[0].lane, c.air_side);
                    assert_eq!(s.clips[0].gain_db, p.clips[0].gain_db + c.gain_db);
                }
            }
        }
        assert_eq!(hash(&p).unwrap(), before);
    }
    #[test]
    fn manifest_roundtrip_rejects_tampering_and_bad_factors() {
        let m = generate(&base(), plan("modality"), Value::Null, Path::new(".")).unwrap();
        validate(&m).unwrap();
        let copy: Manifest = serde_json::from_slice(&serde_json::to_vec(&m).unwrap()).unwrap();
        validate(&copy).unwrap();
        let mut bad = copy;
        bad.trials.swap(0, 1);
        assert!(validate(&bad).is_err());
        let mut bad = m.clone();
        bad.project.settings.routing = [2, 1, 3, 4];
        assert!(validate(&bad).is_err());
        let mut bad = m.plan;
        bad.conditions[0].onset_diff_ms = 100;
        assert!(validate_plan(&bad).is_err());
        bad = plan("modality");
        bad.participants.push("P001".into());
        assert!(validate_plan(&bad).is_err());
        bad = plan("modality");
        bad.conditions[0].id = "../outside".into();
        assert!(validate_plan(&bad).is_err());
    }
}
