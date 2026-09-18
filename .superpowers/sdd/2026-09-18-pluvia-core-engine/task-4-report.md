# Task 4 Report: Linux Telemetry & Measure Engine

## Summary
Implemented the Linux telemetry and measure execution engine in `pluvia-core`. This engine provides native implementations of Rainmeter system telemetry measures and media player integration, reading directly from the Linux kernel (`/proc/stat`, `/proc/meminfo`, `/proc/uptime`, `/proc/net/dev`), POSIX filesystem interfaces (`libc::statvfs`), and desktop D-Bus services (`org.mpris.MediaPlayer2.*` via `zbus`). All measures implement the unified `Measure` trait producing typed `MeasureValue` representations with bidirectional string and numeric conversion.

## Implemented Components

1. **Measure Trait & Values (`crates/pluvia-core/src/measures/mod.rs`)**:
   - `MeasureValue` enum:
     - `String(String)` and `Number(f64)`.
     - `to_string_val()` and `to_number_val()` with integer/floating-point normalization.
     - `as_str()` and `as_f64()` zero-copy accessors.
     - `Display` implementation and standard `From` conversions for primitives (`&str`, `String`, `f64`, `u64`, `i64`).
   - `Measure` trait:
     - `fn update(&mut self) -> MeasureValue`: updates telemetry state and yields current value.
     - `fn get_value(&self) -> MeasureValue`: queries cached value without updating.
   - `create_measure(config: &MeasureConfig) -> Option<Box<dyn Measure>>`:
     - Factory instantiating measures dynamically from parsed `.ini` sections based on `Measure` and `Plugin` keys (`Time`, `CPU`, `PhysicalMemory`, `SwapMemory`, `FreeDiskSpace`, `Uptime`, `NetIn`, `NetOut`, `NetTotal`, and `NowPlaying`).

2. **Time Measure (`crates/pluvia-core/src/measures/time.rs`)**:
   - `TimeMeasure`:
     - Evaluates local time and strftime formatting via `chrono::Local` and `chrono::Utc`.
     - Automatically maps Windows-style non-padded format specifiers (e.g. `%#d`, `%#H`, `%#I`, `%#m`) to POSIX/chrono equivalents (`%-d`, etc.).
     - Configurable timezone support (local, UTC/GMT, and numerical hour offsets).
     - Deterministic testing support via `with_custom_time` and `set_custom_time`.

3. **Linux Kernel Telemetry (`crates/pluvia-core/src/measures/system.rs`)**:
   - `CpuMeasure`:
     - Zero-allocation delta parser reading `/proc/stat` with reusable string buffers.
     - Supports overall system utilization (`cpu`) and individual core tracking (`Processor=1` -> `cpu0`, etc.).
     - Computes active vs. idle deltas including `user`, `nice`, `system`, `iowait`, `irq`, `softirq`, and `steal`.
   - `MemoryMeasure`:
     - Parses `/proc/meminfo` to calculate total, used, free/available, and swap metrics.
     - Supports percentage computation (`used_percent`, `swap_percent`) and raw byte totals.
     - Configurable target paths for mocked container environments and unit tests.
   - `DiskMeasure`:
     - Queries filesystem geometry via `libc::statvfs`.
     - Normalizes Windows drive references (`Drive=C:`, `Drive=C`) to the Linux root mount point (`/`).
     - Computes total, free, used bytes, and utilization percentage.
     - Gracefully handles nonexistent mount points without crashing.
   - `UptimeMeasure`:
     - Reads uptime and idle seconds from `/proc/uptime`.
     - Supports Rainmeter positional format strings (`%4!02d!:%3!02d!:%2!02d!:%1!02d!`) converting seconds into days, hours, minutes, and seconds.
   - `NetMeasure`:
     - Delta bandwidth tracker reading `/proc/net/dev`.
     - Computes transfer rates in bytes/sec for `NetIn` (received), `NetOut` (transmitted), and `NetTotal`.
     - Supports specific network interface filtering or automatic multi-interface summation (excluding loopback `lo`).

4. **MPRIS Media Player Measure (`crates/pluvia-core/src/measures/mpris.rs`)**:
   - `NowPlayingMeasure`:
     - Connects to Linux desktop media players exposing the `org.mpris.MediaPlayer2` specification over session D-Bus using `zbus` blocking APIs.
     - Automatically discovers active players on the bus or targets specific players (`player_name`).
     - Extracts properties: `Title`, `Artist` (from `xesam:artist` string or array), `Album`, `Cover` art URL (`mpris:artUrl`), `State` (0=stopped, 1=playing, 2=paused), `Status` (0=closed, 1=open), `Duration` (from `mpris:length` microseconds), `Position`, and `Progress` (0.0 to 100.0%).
     - Gracefully returns empty/zero defaults when no MPRIS player or D-Bus daemon is running (headless / CI environments).
     - Provides `with_mock_data` and `set_mock_data` for deterministic player testing.

5. **Crate Exports (`crates/pluvia-core/src/lib.rs`)**:
   - Exposed `pub mod measures;`.

## Tests & Verification

- **TDD RED Phase**:
  - Authored `crates/pluvia-core/tests/test_measures.rs` testing all measures and factory creation.
  - Initial `cargo test -p pluvia-core --test test_measures` failed with exit code 101 due to unexported and unwritten `measures` modules.
- **TDD GREEN Phase**:
  - Implemented all modules and achieved clean pass on `cargo test -p pluvia-core --test test_measures` (11 tests passed in 0.01s):
    1. `test_measure_value_conversions`: string to number, number to string formatting, Display and accessor traits.
    2. `test_time_measure_formatting`: strftime strings (`%Y-%m-%d`, `%H:%M`), Windows `%#d` non-padded formatting, and UTC conversion.
    3. `test_cpu_measure_delta_calculation`: real `/proc/stat` read and mocked multi-tick delta calculation.
    4. `test_cpu_measure_per_core`: core-specific parsing (`cpu0`, `cpu1`) with delta computation.
    5. `test_system_memory_measure`: real system `/proc/meminfo` total, used, and used percentage metrics.
    6. `test_mocked_memory_measure`: deterministic verification of memory and swap calculations from mocked `/proc/meminfo`.
    7. `test_disk_measure`: real `statvfs` queries for `/` (free, total, percent) and non-existent path fallback.
    8. `test_uptime_measure`: real system uptime and Rainmeter formatted days/hours/minutes/seconds string.
    9. `test_net_measure`: bandwidth tracking on `NetIn` and `NetOut`.
    10. `test_mpris_now_playing_measure_real_and_mock`: live D-Bus query fallback and full mock validation for Title, Artist, Album, Cover, State, and Progress.
    11. `test_measure_factory_from_config`: instantiating `Time`, `CPU`, `PhysicalMemory`, `FreeDiskSpace`, `Uptime`, `NetIn`, and `Plugin=NowPlaying` measures from `MeasureConfig`.
- **Workspace-wide Verification**:
  - `cargo test` ran and passed all 43 tests across `pluvia-core`, `pluvia-daemon`, and `pluvia-cli` with zero errors.
  - `cargo check --all --tests` compiled with 0 warnings.

## Git Commits
- `fe1bc69`: `feat(core): implement Linux system telemetry and MPRIS measures`
