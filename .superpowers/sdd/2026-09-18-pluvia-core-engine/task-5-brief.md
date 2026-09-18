# Task 5 Brief: Native C++ DLL Plugin Emulators

## Overview
Implement native Rust reimplementations of the most ubiquitous Windows Rainmeter plugins in `pluvia-core`:
1. `crates/pluvia-core/src/plugins/action_timer.rs`: `ActionTimerPlugin` emulating Rainmeter's animation framework (`ActionList`, `Repeat`, `Wait`, variable updates, timer steps).
2. `crates/pluvia-core/src/plugins/audio_level.rs`: `AudioLevelPlugin` emulating audio visualizers and spectrum analyzers (`RMS`, `Peak`, `FFT` frequency bands, attacks, decays, and PipeWire/PulseAudio capture with mockable interface).
3. `crates/pluvia-core/src/plugins/win7_audio.rs`: `Win7AudioPlugin` emulating Windows master audio volume, mute state, and default output sink name.
4. `crates/pluvia-core/src/plugins/process.rs`: `ProcessPlugin` inspecting `/proc` for running process names (returns 1 if active, -1 if absent).
5. `crates/pluvia-core/src/plugins/web_parser.rs`: `WebParserPlugin` executing HTTP queries, regex/string extractions, and cached file downloads.
6. Wire plugins into `crates/pluvia-core/src/measures/mod.rs` so `Plugin=ActionTimer`, `Plugin=AudioLevel`, `Plugin=Win7Audio`, `Plugin=Process`, `Plugin=WebParser` instantiate seamlessly via `create_measure`.
7. Also apply the two telemetry improvements: cache the D-Bus session connection in `NowPlayingMeasure` to avoid per-tick connection churn, and exclude loopback `lo` in `NetMeasure` when `Interface=0`.

## Target Files
- `crates/pluvia-core/Cargo.toml` (add `reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "blocking"] }`, `regex = "1.10"`)
- `crates/pluvia-core/src/plugins/mod.rs`
- `crates/pluvia-core/src/plugins/action_timer.rs`
- `crates/pluvia-core/src/plugins/audio_level.rs`
- `crates/pluvia-core/src/plugins/win7_audio.rs`
- `crates/pluvia-core/src/plugins/process.rs`
- `crates/pluvia-core/src/plugins/web_parser.rs`
- `crates/pluvia-core/src/measures/mpris.rs` (cache connection)
- `crates/pluvia-core/src/measures/system.rs` (exclude `lo` on `Interface=0`)
- `crates/pluvia-core/src/lib.rs` (expose `pub mod plugins;`)
- `crates/pluvia-core/tests/test_plugins.rs`

## Instructions
1. Follow TDD: create failing unit tests in `crates/pluvia-core/tests/test_plugins.rs` covering:
   - `ActionTimerPlugin` action queuing, waits, repeats, and variable mutations.
   - `AudioLevelPlugin` FFT band calculation, RMS/Peak, and decay smoothing.
   - `ProcessPlugin` detecting existing and nonexistent process names.
   - `WebParserPlugin` parsing regex captures from string or HTTP data.
   - `Win7AudioPlugin` volume calculations.
   - `create_measure` factory instantiating all 5 plugins via `Plugin=<Name>`.
2. Verify failure with `cargo test -p pluvia-core --test test_plugins`.
3. Implement plugin modules and factory bindings.
4. Verify all tests pass with `cargo test -p pluvia-core --test test_plugins`.
5. Commit with message: `feat(core): implement native Linux emulators for ActionTimer, AudioLevel, Win7Audio, Process, and WebParser`
6. Write completion report to `.superpowers/sdd/2026-09-18-pluvia-core-engine/task-5-report.md`.
7. Return status under 15 lines.
