//! Bundled first-run sequence. Saved projects are loaded without replacing their timeline.
use crate::{auditory, model::Project};
use std::path::Path;

pub fn project(dir: &Path) -> Result<Project, String> {
    let mut p: Project = serde_json::from_str(include_str!("../assets/starter/orbit-air.json"))
        .map_err(|e| e.to_string())?;
    p.ensure_presets();
    p.draft = p.waves[0].clone();
    p.validate()?;
    let path = auditory::asset_path(dir, &p.audio_assets[0].id)?;
    let bytes = include_bytes!("../assets/starter/orbit-source.wav");
    if std::fs::read(&path).ok().as_deref() != Some(bytes.as_slice()) {
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        let temp = path.with_extension("tmp");
        std::fs::write(&temp, bytes).map_err(|e| e.to_string())?;
        std::fs::rename(temp, path).map_err(|e| e.to_string())?;
    }
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_install_renders_bundled_audio_and_four_correctly_routed_puffs() {
        let dir = std::env::temp_dir().join(format!("aircue-starter-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = project(&dir).unwrap();
        let all = auditory::render_project(&p, &dir, "all").unwrap();
        assert_eq!(all.len(), 336_000 * 4);
        let air = p.render().unwrap();
        for (frame, channels) in all.chunks_exact(4).enumerate() {
            assert_eq!(&channels[2..], &air[frame * 4 + 2..frame * 4 + 4]);
            let ms = frame as f64 / 48.;
            let left = [2550., 5550.].iter().any(|t| ms >= *t && ms < t + 200.);
            let right = [1050., 4050.].iter().any(|t| ms >= *t && ms < t + 200.);
            if !left {
                assert_eq!(channels[2], 0.);
            }
            if !right {
                assert_eq!(channels[3], 0.);
            }
        }
        for (ms, side) in [(1150, 1), (2650, 0), (4150, 1), (5650, 0)] {
            assert!(all[(ms - 1) * 48 * 4 + 2 + side] < -0.12);
            assert!(all[ms * 48 * 4 + 2 + side] > 0.12);
            let energy = |ch: usize| {
                all[(ms - 100) * 48 * 4..(ms + 100) * 48 * 4]
                    .chunks_exact(4)
                    .map(|x| (x[ch] as f64).powi(2))
                    .sum::<f64>()
            };
            assert!(energy(side) > energy(1 - side));
        }
        crate::model::check_peak(&all, 1.).unwrap();
        // Materialization is repeatable, and can repair a missing bundled asset.
        let path = auditory::asset_path(&dir, &p.audio_assets[0].id).unwrap();
        std::fs::remove_file(&path).unwrap();
        project(&dir).unwrap();
        assert_eq!(auditory::render_project(&p, &dir, "all").unwrap(), all);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
