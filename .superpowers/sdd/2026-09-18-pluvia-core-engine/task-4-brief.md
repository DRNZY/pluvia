# Task 4 Brief: Linux Telemetry & Measure Engine

## Overview
Implement the Linux telemetry and measure execution engine in `pluvia-core`:
1. `crates/pluvia-core/src/measures/mod.rs`: `Measure` trait and `MeasureValue` enum (`String(String)`, `Number(f64)`). Measure registry / factory to instantiate measures from `MeasureConfig`.
2. `crates/pluvia-core/src/measures/time.rs`: `TimeMeasure` formatting local time using `strftime` (e.g. `%A`, `%d %B, %Y`, `%H:%M`, `%S`) via `chrono`.
3. `crates/pluvia-core/src/measures/system.rs`:
   - `CpuMeasure`: zero-allocation delta parsing of `/proc/stat` for overall CPU usage percentage and per-core usage.
   - `MemoryMeasure`: parsing `/proc/meminfo` to calculate total bytes, used bytes, free bytes, and percentage.
   - `DiskMeasure` (`FreeDiskSpace`): using `statvfs` on target mount point for total, used, free space and percentage.
   - `UptimeMeasure`: reading `/proc/uptime`.
   - `NetMeasure`: bandwidth delta tracking via `/proc/net/dev`.
4. `crates/pluvia-core/src/measures/mpris.rs`: `NowPlayingMeasure` connecting to MPRIS D-Bus (`org.mpris.MediaPlayer2.*`) for Title, Artist, Album, Cover Art, and Playback Status.
5. Expose `pub mod measures;` in `crates/pluvia-core/src/lib.rs`.

## Target Files
- `crates/pluvia-core/Cargo.toml` (add `chrono = "0.4"`, `libc = "0.2"`, `zbus = { version = "5.0", default-features = false, features = ["tokio"] }` or synchronous zbus blocking)
- `crates/pluvia-core/src/measures/mod.rs`
- `crates/pluvia-core/src/measures/time.rs`
- `crates/pluvia-core/src/measures/system.rs`
- `crates/pluvia-core/src/measures/mpris.rs`
- `crates/pluvia-core/src/lib.rs`
- `crates/pluvia-core/tests/test_measures.rs`

## Instructions
1. Follow TDD: create failing unit tests in `crates/pluvia-core/tests/test_measures.rs` covering:
   - `TimeMeasure` strftime formatting and updates.
   - `CpuMeasure` and `MemoryMeasure` reading real system metrics or mocked `/proc` data.
   - `DiskMeasure` verifying valid disk space statistics.
   - `UptimeMeasure` returning non-zero system uptime.
   - `MeasureValue` string and numeric conversions.
2. Verify failure with `cargo test -p pluvia-core --test test_measures`.
3. Implement `time.rs`, `system.rs`, `mpris.rs`, and `mod.rs`.
4. Verify all tests pass with `cargo test -p pluvia-core --test test_measures`.
5. Commit with message: `feat(core): implement Linux system telemetry and MPRIS measures`
6. Write completion report to `.superpowers/sdd/2026-09-18-pluvia-core-engine/task-4-report.md`.
7. Return status under 15 lines.
