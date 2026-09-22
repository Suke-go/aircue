# AirCue 0.6.0 verification

## Built-in rotating audio and air-puff starter

- Correction against the user-supplied January 19, 2023 PDF, Fig.5(a)/§7.1: the default now uses the b=1 negative-100-ms/positive-100-ms rectangular drive. Positive transitions align to the audio side passes; drive starts are 100 ms earlier. The new bipolar part supports 200 ms while existing parts retain their 80 ms bounds. A 13th built-in wave exposes it in the editor.
- Updated validation: 46 ASIO-enabled Rust tests and 22 JavaScript tests passed, including exact phase samples, zero sample sum, onset routing, rendered headphone dominance, both editor limits, preset migration and saved custom-wave preservation. These checks do not establish ±10 V at the amplifier, membrane displacement, air pressure, or physical arrival synchronization.
- First-run startup materializes the bundled source audio and opens a seven-second timeline with two audio orbits and four alternating air cues. Saved projects retain their timeline. Startup now opens the timeline view.
- ASIO-enabled Rust: 45 tests passed. The new test renders bundled media in a fresh data directory, checks all four air onsets/routes and headphone side dominance, checks clipping, and verifies recovery after removing the bundled asset. JavaScript: 20 tests and syntax/version checks passed.
- The local working application's previous data was backed up before selecting the new starter; its output settings and wave library were preserved. The original 12+48 ms adaptation was superseded by the correction above. Physical output and arrival latency remain unverified.

## Follow-up: experiments without Unity

- Standard Experiment Bundle now contains plain JSON and WAV, with no Unity runtime/editor files. Unity is an explicit optional adapter. Both exports use one PCM/metadata writer; a parity test checks identical manifests, checksums and condition/calibration WAVs.
- Added AirCue-native participant sessions, four-channel calibration gate, primary responses followed by ratings, aborts, durable JSONL and exclusive access to project edits/audio playback. The native runner uses software frame-progress estimates, not physical onset timestamps.
- ASIO-enabled Rust: 44 tests passed. Added browser integer/float JSON roundtrip regression, adapter parity, response timing before ratings, duplicate/early answer rejection, interrupted playback and log-write failure tests. JavaScript: 20 tests and syntax/version checks passed.
- Native Windows UI with isolated data: standard WAV/JSON export and optional Unity section, 6-condition/24-trial generation, and participant selection were exercised. A native manifest roundtrip failure was found and corrected: JavaScript serializes integral floating values as integers; validation now compares JSON numbers numerically while retaining exact hashes, fields and order.
- Full audible trial/response sessions and physical four-channel output remain untested. No Python/PsychoPy-specific execution adapter is included. Native sessions do not resume after interruption; a new session starts at the first trial.
- Final ASIO-enabled release/NSIS build succeeded. The built CLI exported a neutral 24-trial bundle successfully. After the JSON fix, the native generated manifest passed validation and reached the expected missing-four-channel-device gate without playback.

- Baseline: private repository main and v0.5.0 at 4c0edf3a2444d1f48980ccd068a7a45638212c16.
- Windows MSVC: 39 Rust tests passed with ASIO enabled. New coverage includes deterministic participant/block orders and repetitions, signed onsets, routing and gain transformations, strict manifest roundtrip and tampering rejection, case-insensitive condition IDs, frozen media rejection, clipping cleanup, actual exported PCM and single-channel calibration.
- JavaScript: 20 tests passed, including all simple presets, non-destructive custom recognition, a short custom path deviation, legacy long-arc rotation across the seam, distance limits, source preservation and A/B copies. Syntax and version checks passed.
- cargo fmt --check and git diff --check passed.
- ASIO-enabled Tauri release and NSIS installer built successfully. No installer was installed and no tag/release was created.
- Native Windows UI was exercised with isolated fixture data: simple default, detail editing, return to custom without losing elevation, and experiment generation (6 conditions, 4 repetitions, 24 trials). Final release visual check covered the larger ring beside the controls and separate experiment workflow. Audible preview and physical hardware output were not exercised.
- Unity 6000.4.3f1 in an isolated project: existing Sequence import verification and new Experiment Bundle verification both exited successfully. Markers AIRCUE_UNITY_VERIFIED and AIRCUE_EXPERIMENT_VERIFIED were emitted. Verified C# compilation, prefabs/subassets, PCM equality, signed ±100 ms starts, four isolated calibration channels, reimport references, calibration gate, configuration-change invalidation and session/calibration log provenance. Verification sources are unity/Tests/Editor/AirCueImportVerification.cs and AirCueExperimentVerification.cs; they are not distributed with bundles.
- The first sandboxed Unity attempt failed to connect to the local licensing client. The normal-permission isolated verification completed successfully. ASIO used the already-present SDK after verifying the repository's pinned ZIP checksum; no SDK or dependency version changed.
- Limits: no physical localization/comfort assessment, Rubix44/air-puff actuation, measured physical onset/latency, full live trial-response session or long-duration drift test. Unity response UI and breaks remain study-owned. Experiment presets accept one audio and one puff clip, and JSON factor editing rather than a full factorial-table GUI.

---

# AirCue 0.5.0 verification

- Windows MSVC / Rust 1.95.0: 33 Rust unit tests passed. The ASIO-enabled Tauri release build and NSIS installer completed successfully.
- JavaScript: 13 spatial-model/history tests passed; syntax and version checks passed.
- New Rust coverage includes strict experiment request fields and IDs, newline-delimited multiple requests, malformed JSON rejection, and four-channel scheduling silence without modifying the cue.
- Browser UI checked using an isolated command adapter: stopped, listening, prepared, trial-playing and stopped states; timeline editing invalidates preparation; 760 px responsive layout. No browser console warnings or errors were reported. This adapter is not shipped.
- Unity 6000.4.3f1: the actual Rust CLI export of tests/unity-project.json was imported in a new isolated project. C# compilation, prefab and subasset generation, PCM channel equality, start frames, routing permutation, reimport references, and AirCueExperimentClient defaults passed. The batch exited with code 0 and AIRCUE_UNITY_VERIFIED in its log.
- Unity verification code: unity/Tests/Editor/AirCueImportVerification.cs. It is not included in exported folders. GitHub CI does not run Unity because no Unity license is configured there.
- Native playback, Unity-to-AirCue timing on hardware, acoustic localization, Rubix44 four-channel physical output, driver downmix behavior and physical air-puff strength were not verified. Unity imports, client compilation and audio sample content were verified without playing sound.
- GitHub Actions runs Rust and JavaScript tests, version checks, NSIS packaging and corresponding-source packaging before publishing the tag release.
