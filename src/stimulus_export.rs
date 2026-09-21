//! Platform-independent baked WAV and JSON sequence writer.
use crate::{model, write_wav};
use serde_json::json;
use std::path::Path;

pub fn write_sequence(p: &model::Project, rendered: &[f32], sequence: &Path) -> Result<(), String> {
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
