# Task 5 Report: Native C++ DLL Plugin Emulators

## Summary
Implemented native Rust emulators in `pluvia-core` for the most ubiquitous Windows Rainmeter C++ DLL plugins: `ActionTimer`, `AudioLevel`, `Win7Audio`, `Process`, and `WebParser`. These plugins integrate seamlessly with the `Measure` trait and the `create_measure` factory, enabling Rainmeter skins to execute animations, spectrum visualizers, volume controls, process presence checks, and HTTP/regex scraping natively on Linux without Wine or Windows binaries. In addition, cached D-Bus connections in `NowPlayingMeasure` to prevent socket churn and ensured network loopback interface `lo` is properly filtered when `Interface=0` or `Interface=all`.

## Implemented Components

1. **ActionTimer Plugin (`crates/pluvia-core/src/plugins/action_timer.rs`)**:
   - `ActionTimerPlugin`:
     - Emulates Rainmeter's animation framework and timer sequence execution.
     - Parses pipe-separated `ActionListN` pipelines containing `Wait <ms>`, `Repeat <ActionName>, <WaitMs>, <Count>`, and direct named action executions.
     - Evaluates Rainmeter bangs such as `[!SetVariable VarName VarValue]`, supporting nested `#Var#` expansions, mathematical formulas in parentheses via `eval_formula`, and variable mutations.
     - Supports step-based deterministic execution (`step(delta_ms)`) as well as real-time tick progression in `update()`.
     - Supports standard bang commands via `command()` (`"Execute <N>"`, `"Stop <N>"`, `"Stop"`).
     - Returns `1.0` while actions are executing and `0.0` when idle.

2. **AudioLevel Plugin (`crates/pluvia-core/src/plugins/audio_level.rs`)**:
   - `AudioLevelPlugin`:
     - Emulates audio visualizers and spectrum analyzers (`Type=RMS`, `Peak`, `FFT`, `BandFreq`).
     - Includes pure Rust radix-2 in-place Cooley-Tukey FFT with Hann windowing for frequency spectrum analysis.
     - Maps raw FFT bin magnitudes into configurable logarithmic frequency bands (`freq_min` to `freq_max`).
     - Computes RMS ($\sqrt{\frac{1}{N}\sum x_i^2}$) and Peak ($\max |x_i|$) amplitudes.
     - Implements exponential attack and decay filters for smooth visualizer animations.
     - Supports stereo audio routing (`Left`, `Right`, `Avg`, `Sum`) via `feed_stereo` and mono samples via `feed_samples`.

3. **Win7Audio Plugin (`crates/pluvia-core/src/plugins/win7_audio.rs`)**:
   - `Win7AudioPlugin`:
     - Emulates Windows master audio volume, mute state, and default output sink name.
     - Supports commands: `"SetVolume <N>"`, `"ChangeVolume <+/-N>"`, `"ToggleMute"`, `"SetMute 1/0"`.
     - Automatically clamps volume percentages between `0.0` and `100.0`.
     - Returns `-1.0` when muted (matching Rainmeter specification) and `0.0..100.0` when unmuted.
     - Interacts with Linux audio servers (`wpctl` for PipeWire, `pactl` for PulseAudio) with mockable overrides (`with_mock`) for deterministic testing.

4. **Process Plugin (`crates/pluvia-core/src/plugins/process.rs`)**:
   - `ProcessPlugin`:
     - Inspects `/proc/<pid>/comm` and `/proc/<pid>/cmdline` for running processes on Linux.
     - Matches process names case-insensitively, automatically handling Windows `.exe` suffixes (e.g. `notepad.exe` matches `notepad` or `/usr/bin/notepad.exe`).
     - Returns `1.0` if active and `-1.0` if absent.
     - Configurable target directory (`with_proc_dir`) for deterministic mocking in tests.

5. **WebParser Plugin (`crates/pluvia-core/src/plugins/web_parser.rs`)**:
   - `WebParserPlugin`:
     - Executes HTTP/HTTPS requests (using `reqwest::blocking` with `rustls-tls`), `file://` URIs, and local file reads.
     - Applies regex pattern matching via `regex::Regex` and extracts capture groups based on `StringIndex` (1-indexed).
     - Supports downloading and caching files to disk via `download_to_file`.
     - Provides `with_mock_data` and `set_mock_data` for isolated testing.

6. **Measure Factory Integration (`crates/pluvia-core/src/measures/mod.rs`)**:
   - Updated `create_measure` to recognize both `Plugin=<Name>` (with or without `.dll`) and direct `Measure=<Name>` for all 5 plugins: `ActionTimer`, `AudioLevel`, `Win7Audio`, `Process`, and `WebParser`.

7. **Telemetry Improvements (`crates/pluvia-core/src/measures/`)**:
   - **`mpris.rs`**: Cached the `zbus::blocking::Connection` in `NowPlayingMeasure` to eliminate D-Bus socket churn on every update tick.
   - **`system.rs`**: Excluded the loopback interface `lo` when `Interface=0` or `Interface=all` in `NetMeasure`.

## Tests & Verification

- **TDD RED Phase**:
  - Authored `crates/pluvia-core/tests/test_plugins.rs`.
  - Verified compilation failure (`could not find plugins in pluvia_core`) via `cargo test -p pluvia-core --test test_plugins`.
- **TDD GREEN Phase**:
  - Implemented all 5 plugins and wired them into `create_measure` and `lib.rs`.
  - All 6 tests in `test_plugins` passed cleanly:
    1. `test_action_timer_plugin_queuing_waits_repeats_and_variables`: verified queuing, sequential waits, repetitions, formula expansions in `[!SetVariable]`, and bang commands (`Execute`, `Stop`).
    2. `test_audio_level_plugin_fft_rms_peak_and_smoothing`: verified RMS and Peak on DC signals, 1000 Hz pure tone FFT peak detection across 8 logarithmic bands, decay smoothing on silence, and stereo channel extraction.
    3. `test_process_plugin_detects_processes`: verified detection of mocked processes with and without `.exe` extension, and negative response for missing processes.
    4. `test_web_parser_plugin_regex_extraction_and_mock_data`: verified regex capture groups (city and temperature) and file download caching.
    5. `test_win7_audio_plugin_volume_calculations`: verified volume clamping (0..100), mute toggling (-1.0), and bang commands.
    6. `test_create_measure_factory_plugins`: verified `create_measure` instantiates all 5 plugins via `Measure=Plugin` and direct `Measure=<PluginName>`.
  - Added `test_net_measure_filters_loopback_on_all_and_zero` in `test_measures.rs` confirming loopback `lo` is excluded on `Interface=0` and `Interface=all`.
- **Workspace-wide Verification**:
  - `cargo test --all` ran and passed all 50 tests across the workspace with 0 failures.
  - `cargo check --all --tests` compiled with zero warnings.

## Git Commits
- `a343ecc`: `feat(core): implement native Linux emulators for ActionTimer, AudioLevel, Win7Audio, Process, and WebParser`
