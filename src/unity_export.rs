use crate::{auditory, model, write_wav};
use serde_json::json;
use std::path::{Path, PathBuf};

const FILES: &[(&str, &str)] = &[
    (
        "docs/EXPERIMENT_DESIGN.md",
        include_str!("../docs/EXPERIMENT_DESIGN.md"),
    ),
    (
        "Runtime/AirCueExperimentSet.cs",
        include_str!("../unity/Runtime/AirCueExperimentSet.cs"),
    ),
    (
        "Runtime/AirCueExperimentRunner.cs",
        include_str!("../unity/Runtime/AirCueExperimentRunner.cs"),
    ),
    (
        "Editor/AirCueExperimentImporter.cs",
        include_str!("../unity/Editor/AirCueExperimentImporter.cs"),
    ),
    (
        "Runtime/AirCueSequence.cs",
        include_str!("../unity/Runtime/AirCueSequence.cs"),
    ),
    (
        "Runtime/AirCuePlayer.cs",
        include_str!("../unity/Runtime/AirCuePlayer.cs"),
    ),
    (
        "Runtime/AirCueExperimentClient.cs",
        include_str!("../unity/Runtime/AirCueExperimentClient.cs"),
    ),
    (
        "Editor/AirCueImporter.cs",
        include_str!("../unity/Editor/AirCueImporter.cs"),
    ),
    ("README.md", include_str!("../unity/README.md")),
    (
        "docs/EXPERIMENT_SYNC.md",
        include_str!("../docs/EXPERIMENT_SYNC.md"),
    ),
    ("LICENSE", include_str!("../LICENSE")),
    (
        "THIRD_PARTY_HRTF.txt",
        include_str!("../THIRD_PARTY_HRTF.txt"),
    ),
];

pub fn export(p: &model::Project, source: &Path, destination: &Path) -> Result<PathBuf, String> {
    if p.clips.is_empty() && p.audio_clips.is_empty() {
        return Err("タイムラインに音声または波形を配置してください".into());
    }
    let rendered = auditory::render_project(p, source, "all")?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let root = destination.join(format!("AirCue-{stamp}"));
    // A new directory is required: existing exports and Unity assets are never replaced here.
    std::fs::create_dir(&root).map_err(|e| e.to_string())?;
    let result = write_package(p, &rendered, &root, &stamp.to_string());
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&root);
    }
    result.map(|_| root)
}

pub(crate) fn write_package(
    p: &model::Project,
    rendered: &[f32],
    root: &Path,
    id: &str,
) -> Result<(), String> {
    let package = root.join("AirCueUnity");
    for (name, text) in FILES {
        let path = package.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::write(path, text).map_err(|e| e.to_string())?;
    }
    let sequence = package.join("Sequences").join(format!("Sequence-{id}"));
    std::fs::create_dir_all(&sequence).map_err(|e| e.to_string())?;
    let routing = p.settings.routing;
    let stereo: Vec<_> = rendered
        .chunks_exact(4)
        .flat_map(|f| [f[routing[0] - 1], f[routing[1] - 1]])
        .collect();
    write_wav(&sequence.join("headphones.wav"), &stereo, 2)?;
    write_wav(&sequence.join("routed-4ch.wav"), rendered, 4)?;
    for (i, name) in [(2, "air-left.wav"), (3, "air-right.wav")] {
        let samples: Vec<_> = rendered
            .chunks_exact(4)
            .map(|f| f[routing[i] - 1])
            .collect();
        write_wav(&sequence.join(name), &samples, 1)?;
    }
    let mut markers: Vec<_> = p.clips.iter().map(|c| {
        let wave = p.waves.iter().find(|w| w.id == c.wave_id).unwrap();
        json!({"id":c.id,"name":wave.name,"lane":c.lane,
            "startFrame":model::ms_frame(c.start_ms),"durationFrames":model::ms_frame(wave.duration())})
    }).chain(p.audio_clips.iter().map(|c| json!({"id":c.id,"name":c.name,"lane":"audio",
        "startFrame":model::ms_frame(c.start_ms),"durationFrames":model::ms_frame(c.design.duration_ms)}))).collect();
    markers.sort_by_key(|m| m["startFrame"].as_u64().unwrap());
    let metadata = json!({
        "schemaVersion":1,"generatorVersion":env!("CARGO_PKG_VERSION"),
        "sampleRate":model::RATE,"frames":rendered.len()/4,"routing":routing,
        "markers":markers,"audioClips":p.audio_clips,
        "headphoneRendering":"baked-stereo","distanceCm":p.settings.distance_cm
    });
    std::fs::write(
        sequence.join("Sequence.aircueseq"),
        serde_json::to_vec_pretty(&metadata).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unity_bundle_keeps_timing_routes_and_stereo_bake() {
        let dir = std::env::temp_dir().join(format!("aircue-unity-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut p = model::Project::default();
        p.duration_ms = 200.;
        p.settings.routing = [3, 4, 2, 1];
        p.clips.push(model::Clip {
            id: "air".into(),
            wave_id: "single".into(),
            lane: "left".into(),
            start_ms: 100.,
            gain_db: 0.,
        });
        p.audio_clips.push(auditory::AudioClip {
            id: "audio".into(),
            name: "test".into(),
            start_ms: 20.,
            gain_db: -3.,
            design: auditory::Design {
                duration_ms: 100.,
                spatial: true,
                azimuth: 90.,
                ..Default::default()
            },
        });
        let output = export(&p, &dir, &dir).unwrap();
        let sequences = output.join("AirCueUnity/Sequences");
        let sequence = std::fs::read_dir(sequences)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let meta: serde_json::Value =
            serde_json::from_slice(&std::fs::read(sequence.join("Sequence.aircueseq")).unwrap())
                .unwrap();
        assert_eq!(meta["frames"], 9600);
        assert_eq!(meta["routing"], json!([3, 4, 2, 1]));
        assert_eq!(meta["markers"][0]["startFrame"], 960);
        let samples = |name| {
            let mut reader = hound::WavReader::open(sequence.join(name)).unwrap();
            assert_eq!(reader.spec().sample_rate, 48000);
            reader
                .samples::<i32>()
                .map(Result::unwrap)
                .collect::<Vec<_>>()
        };
        let four = samples("routed-4ch.wav");
        let stereo = samples("headphones.wav");
        let air = samples("air-left.wav");
        assert_eq!(four.len(), 9600 * 4);
        assert_eq!(stereo.len(), 9600 * 2);
        for i in 0..9600 {
            assert_eq!(stereo[i * 2], four[i * 4 + 2]);
            assert_eq!(stereo[i * 2 + 1], four[i * 4 + 3]);
            assert_eq!(air[i], four[i * 4 + 1]);
        }
        assert!(air[..4800].iter().all(|v| *v == 0));
        assert!(air[4800..].iter().any(|v| *v != 0));
        assert!(samples("air-right.wav").iter().all(|v| *v == 0));
        assert!(output.join("AirCueUnity/Editor/AirCueImporter.cs").exists());
        assert!(output
            .join("AirCueUnity/Runtime/AirCueExperimentClient.cs")
            .exists());
        assert!(output.join("AirCueUnity/docs/EXPERIMENT_SYNC.md").exists());
        std::fs::remove_dir_all(output).unwrap();
        std::fs::remove_dir(&dir).unwrap();
    }
    #[test]
    fn invalid_or_empty_exports_do_not_create_partial_packages() {
        let dir = std::env::temp_dir();
        let mut p = model::Project::default();
        assert!(export(&p, &dir, &dir).unwrap_err().contains("配置"));
        p.audio_clips.push(auditory::AudioClip {
            id: "invalid".into(),
            name: "missing".into(),
            start_ms: 0.,
            gain_db: 0.,
            design: auditory::Design {
                source: "file".into(),
                ..Default::default()
            },
        });
        assert!(export(&p, &dir, &dir).is_err());
    }
}
