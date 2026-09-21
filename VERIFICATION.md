# AirCue 0.2.0 verification

- Windows MSVC / Rust 1.95.0 / ASIO enabled: 25 unit tests passed.
- Tests cover old project loading, waveform synthesis, gain limits, routing, 2ch fallback, resampling, WAV export, audio import/trim, portable assets, HRTF direction, interpolation and source motion.
- Release executable compiled successfully and replaced the canonical AirCue.exe.
- Frontend checked in the in-app browser with an isolated Tauri command adapter: four views in one window, spatial switch, azimuth/elevation, orbit controls, timeline insertion, clip editing, stereo bypass, imported-asset UI.
- The UI adapter is test-only and is not included in the application.
- Native file dialog interaction, acoustic localization, Rubix44 simultaneous four-channel playback and physical air-puff strength were not verified in this release session.
- GitHub Actions runs the Rust tests, JavaScript syntax/version checks, NSIS packaging and corresponding-source packaging before publishing a tag release.
- GitHub Windows runner: unit tests and NSIS build passed; AirCue-windows-x64 artifact uploaded in run 35562676413.
