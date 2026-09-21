# AirCue 0.4.0 verification

- Windows MSVC / Rust 1.95.0 / ASIO enabled: 30 Rust unit tests passed.
- JavaScript: 13 spatial-model/history tests passed; syntax and version checks passed.
- New coverage includes path mirroring/reversal, equal timing and loop closure, independent undo/redo snapshots, Unity WAV channel extraction and shared frame timing, and rejection of invalid exports.
- Browser UI checked using an isolated command adapter: mirror, undo/redo, dragging a height/time point, undoing the complete drag, timeline placement and Unity export instructions. No browser console errors were reported. This adapter is not shipped.
- Unity 6000.4.3f1: the actual Rust CLI export of tests/unity-project.json was imported in an isolated project. C# compilation, prefab and subasset generation, PCM channel equality, start frames, routing permutation and reimport references passed. The batch exited with code 0 and AIRCUE_UNITY_VERIFIED in its log.
- Unity verification code: unity/Tests/Editor/AirCueImportVerification.cs. It is not included in exported folders. GitHub CI does not run Unity because no Unity license is configured there.
- Native playback, acoustic localization, Rubix44 four-channel physical output, driver downmix behavior and physical air-puff strength were not verified. Unity imports and audio sample content were verified without playing sound.
- GitHub Actions runs Rust and JavaScript tests, version checks, NSIS packaging and corresponding-source packaging before publishing the tag release.
