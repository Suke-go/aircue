use super::*;

fn temp_dir() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "aircue-auditory-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&p).unwrap();
    p
}

#[test]
fn legacy_project_loads_with_audio_disabled() {
    let mut json = serde_json::to_value(Project::default()).unwrap();
    for key in ["audioAssets", "audioClips", "audioDraft"] {
        json.as_object_mut().unwrap().remove(key);
    }
    let p: Project = serde_json::from_value(json).unwrap();
    p.validate().unwrap();
    assert!(p.audio_clips.is_empty());
    assert!(!p.audio_draft.spatial);
}

#[test]
fn hrtf_right_source_is_louder_and_earlier_in_right_ear() {
    let right = hrtf(90., 0.);
    let left = hrtf(-90., 0.);
    let energy = |ch: usize| right.iter().map(|f| f[ch] * f[ch]).sum::<f32>();
    assert!(energy(1) > energy(0));
    let peak = |ch: usize| {
        right
            .iter()
            .enumerate()
            .max_by(|a, b| a.1[ch].abs().total_cmp(&b.1[ch].abs()))
            .unwrap()
            .0
    };
    assert!(peak(1) < peak(0));
    for (r, l) in right.iter().zip(left) {
        assert_eq!(*r, [l[1], l[0]]);
    }
    assert_ne!(hrtf(0., 0.), hrtf(0., 40.));
}

#[test]
fn hrtf_interpolation_covers_poles_and_wrapped_angles() {
    for el in [-40., -35., 0., 17., 45., 85., 90.] {
        for az in [-540., -179., -90., 0., 42., 179., 540.] {
            let h = hrtf(az, el);
            assert_eq!(h.len(), 140);
            assert!(h.iter().flatten().all(|v| v.is_finite()));
        }
    }
}

#[test]
fn stereo_import_trim_bypass_and_portable_assets_roundtrip() {
    let dir = temp_dir();
    let src = dir.join("test.wav");
    let mut w = hound::WavWriter::create(
        &src,
        hound::WavSpec {
            channels: 2,
            sample_rate: 44100,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .unwrap();
    for _ in 0..4410 {
        w.write_sample(8192i16).unwrap();
        w.write_sample(-4096i16).unwrap();
    }
    w.finalize().unwrap();
    let asset = import_file(&src, &dir).unwrap();
    assert_eq!(asset.frames, 4800);
    assert_eq!(asset.source_channels, 2);
    let d = Design {
        source: "file".into(),
        asset_id: Some(asset.id.clone()),
        offset_ms: 20.,
        duration_ms: 40.,
        level_db: 0.,
        ..Default::default()
    };
    let audio = render_design(&d, &[asset.clone()], &dir).unwrap();
    assert_eq!(audio.len(), 3840);
    assert!((audio[1000] - 0.25).abs() < 0.0001);
    assert!((audio[1001] + 0.125).abs() < 0.0001);
    let mut p = Project::default();
    p.audio_assets.push(asset);
    p.audio_draft = d.clone();
    p.validate().unwrap();
    let portable = dir.join("portable.media");
    copy_assets(&p, &dir, &portable).unwrap();
    assert_eq!(
        audio,
        render_design(&d, &p.audio_assets, &portable).unwrap()
    );
    assert!(Design {
        offset_ms: 90.,
        ..d
    }
    .validate(&p.audio_assets)
    .is_err());
    assert!(asset_path(&dir, "../../escape").is_err());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn mixed_timeline_routes_audio_and_air_independently() {
    let mut p = Project::default();
    p.duration_ms = 100.;
    p.settings.routing = [3, 4, 1, 2];
    p.clips.push(model::Clip {
        id: "air".into(),
        wave_id: "single".into(),
        lane: "left".into(),
        start_ms: 0.,
        gain_db: 0.,
    });
    p.audio_clips.push(AudioClip {
        id: "audio".into(),
        name: "tone".into(),
        start_ms: 10.,
        gain_db: 0.,
        design: Design {
            source: "sine".into(),
            duration_ms: 50.,
            ..Default::default()
        },
    });
    let all = render_project(&p, Path::new("."), "all").unwrap();
    let air = render_project(&p, Path::new("."), "air").unwrap();
    let audio = render_project(&p, Path::new("."), "audio").unwrap();
    assert!(all.chunks_exact(4).any(|f| f[0] != 0. && f[2] != 0.));
    for ((a, b), c) in all.iter().zip(air).zip(audio) {
        assert_eq!(*a, b + c);
    }
    assert!(all.chunks_exact(4).all(|f| f[1] == 0. && f[2] == f[3]));
    p.audio_clips[0].gain_db = 60.;
    assert!(p.validate().is_err());
}

#[test]
fn spatial_motion_and_gain_validation() {
    let d = Design {
        duration_ms: 80.,
        spatial: true,
        motion: "orbit".into(),
        speed: 360.,
        ..Default::default()
    };
    let moving = render_design(&d, &[], Path::new(".")).unwrap();
    let fixed = render_design(
        &Design {
            motion: "fixed".into(),
            ..d.clone()
        },
        &[],
        Path::new("."),
    )
    .unwrap();
    assert_ne!(moving, fixed);
    assert_eq!(moving.len(), 7680);
    assert!(render_design(
        &Design {
            level_db: 12.,
            source: "square".into(),
            spatial: false,
            ..d.clone()
        },
        &[],
        Path::new(".")
    )
    .is_err());
    assert!(Design {
        azimuth: f64::NAN,
        ..d
    }
    .validate(&[])
    .is_err());
}

#[test]
fn spatial_positions_match_shared_editor_fixtures() {
    let fixtures: serde_json::Value =
        serde_json::from_str(include_str!("../tests/spatial-cases.json")).unwrap();
    for case in fixtures["cases"].as_array().unwrap() {
        let mut json = serde_json::to_value(Design::default()).unwrap();
        for (key, value) in case["patch"].as_object().unwrap() {
            json[key] = value.clone();
        }
        let d: Design = serde_json::from_value(json).unwrap();
        d.validate(&[]).unwrap();
        let p = position_at(&d, case["at"].as_f64().unwrap());
        for (i, v) in [p.azimuth, p.elevation, p.distance_m].iter().enumerate() {
            assert!(
                (v - case["expected"][i].as_f64().unwrap()).abs() < 1e-8,
                "{}",
                case["name"]
            );
        }
    }
}

#[test]
fn path_validation_roundtrip_and_legacy_audio_migration() {
    let mut old = serde_json::to_value(Design::default()).unwrap();
    old.as_object_mut().unwrap().remove("keyframes");
    let old: Design = serde_json::from_value(old).unwrap();
    assert!(old.keyframes.is_empty());
    let mut d = Design {
        motion: "path".into(),
        keyframes: vec![
            Keyframe {
                at: 0.,
                azimuth: -90.,
                elevation: 0.,
                distance_m: 1.,
            },
            Keyframe {
                at: 0.5,
                azimuth: 0.,
                elevation: 90.,
                distance_m: 2.,
            },
            Keyframe {
                at: 1.,
                azimuth: 90.,
                elevation: 0.,
                distance_m: 1.,
            },
        ],
        ..old
    };
    d.validate(&[]).unwrap();
    assert_eq!(
        d,
        serde_json::from_str::<Design>(&serde_json::to_string(&d).unwrap()).unwrap()
    );
    d.keyframes[1].at = 0.;
    assert!(d.validate(&[]).is_err());
    d.keyframes[1].at = 0.5;
    d.keyframes[1].elevation = f64::NAN;
    assert!(d.validate(&[]).is_err());
    d.keyframes.clear();
    assert!(d.validate(&[]).is_err());
}

#[test]
fn path_distance_is_applied_in_render_and_stereo_bypass_ignores_path() {
    let d = Design {
        spatial: true,
        source: "sine".into(),
        duration_ms: 80.,
        ..Default::default()
    };
    let near = render_design(&d, &[], Path::new(".")).unwrap();
    let mut path = Design {
        motion: "path".into(),
        keyframes: vec![
            Keyframe {
                at: 0.,
                azimuth: 0.,
                elevation: 0.,
                distance_m: 1.,
            },
            Keyframe {
                at: 1.,
                azimuth: 0.,
                elevation: 0.,
                distance_m: 4.,
            },
        ],
        ..d.clone()
    };
    let moving = render_design(&path, &[], Path::new(".")).unwrap();
    for (i, (a, b)) in near.chunks_exact(2).zip(moving.chunks_exact(2)).enumerate() {
        let distance = 1. + 3. * i as f32 / (near.len() / 2) as f32;
        assert!((a[0] - b[0] * distance).abs() < 1e-6);
    }
    path.spatial = false;
    let dry = Design {
        spatial: false,
        ..d
    };
    assert_eq!(
        render_design(&path, &[], Path::new(".")).unwrap(),
        render_design(&dry, &[], Path::new(".")).unwrap()
    );
}
