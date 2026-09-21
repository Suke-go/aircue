# AirCue 0.3.0 verification

- Windows MSVC / Rust 1.95.0 / ASIO enabled: 28 Rust unit tests passed.
- JavaScript: 10 spatial-model tests passed; all four production scripts passed syntax checks; application and lockfile versions agree.
- Shared fixtures compare JavaScript and Rust positions across the rear seam, intermediate points, orbit, legacy angular sweep and the exact half-turn case.
- Rust tests also cover path validation and old-project compatibility, rendered distance attenuation, stereo bypass, import/trim, portable assets, routing, waveform synthesis and gain limits.
- Frontend checked in a hidden browser using an isolated Tauri command adapter: spatial presets, point insertion, map drag, keyboard entry of passage time, timeline insertion, clip duplication, clip update/reopen, persisted paths, spatial bypass and file-trim duration clamping.
- The UI adapter is test-only and is not included in the application. It does not verify native audio playback or the native file dialog.
- Local ASIO release and NSIS installer compiled successfully. The canonical AirCue.exe reports version 0.3.0. A silent native diagnostics launch succeeded with ASIO support enabled and available WASAPI devices listed.
- Native file dialog interaction, acoustic localization, Rubix44 simultaneous four-channel playback and physical air-puff strength were not verified in this release session.
- GitHub Actions runs Rust and JavaScript tests, version checks, NSIS packaging and corresponding-source packaging before publishing a tag release. Run results are recorded in GitHub Actions.
