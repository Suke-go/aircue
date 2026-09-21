# AirCue 0.5.0 verification

- Windows MSVC / Rust 1.95.0: 33 Rust unit tests passed. The ASIO-enabled Tauri release build and NSIS installer completed successfully.
- JavaScript: 13 spatial-model/history tests passed; syntax and version checks passed.
- New Rust coverage includes strict experiment request fields and IDs, newline-delimited multiple requests, malformed JSON rejection, and four-channel scheduling silence without modifying the cue.
- Browser UI checked using an isolated command adapter: stopped, listening, prepared, trial-playing and stopped states; timeline editing invalidates preparation; 760 px responsive layout. No browser console warnings or errors were reported. This adapter is not shipped.
- Unity 6000.4.3f1: the actual Rust CLI export of tests/unity-project.json was imported in a new isolated project. C# compilation, prefab and subasset generation, PCM channel equality, start frames, routing permutation, reimport references, and AirCueExperimentClient defaults passed. The batch exited with code 0 and AIRCUE_UNITY_VERIFIED in its log.
- Unity verification code: unity/Tests/Editor/AirCueImportVerification.cs. It is not included in exported folders. GitHub CI does not run Unity because no Unity license is configured there.
- Native playback, Unity-to-AirCue timing on hardware, acoustic localization, Rubix44 four-channel physical output, driver downmix behavior and physical air-puff strength were not verified. Unity imports, client compilation and audio sample content were verified without playing sound.
- GitHub Actions runs Rust and JavaScript tests, version checks, NSIS packaging and corresponding-source packaging before publishing the tag release.
