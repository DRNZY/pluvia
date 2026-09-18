# Task 8 Brief: UNIX Domain Socket JSON-RPC 2.0 Server & CLI

## Overview
Implement the daemon runtime orchestration, asynchronous UNIX domain socket JSON-RPC 2.0 server, and command-line controller (`pluvia-cli`):
1. `crates/pluvia-daemon/src/runtime.rs`:
   - `SkinRuntime` managing active skin instances, update ticker loops (`Update=...`), measure updates, meter rendering via `MeterRenderer`, surface commit, and `AlphaHitMask` click-through submission.
   - Ensures damage rects are cleared per frame to prevent memory accumulation.
2. `crates/pluvia-daemon/src/ipc.rs`:
   - Async Tokio UNIX domain socket server implementing strict JSON-RPC 2.0.
   - Methods: `pluvia.ping`, `pluvia.loadSkin`, `pluvia.unloadSkin`, `pluvia.listSkins`, `pluvia.getState`, `pluvia.refreshSkin`, `pluvia.refreshAll`, `pluvia.setVariable`, `pluvia.importPackage`.
   - Pub/Sub notifications: `notify.skinLoaded`, `notify.skinUnloaded`, `notify.packageExtractProgress`.
3. `crates/pluvia-cli/src/main.rs` & `client.rs`:
   - Command-line interface with subcommands: `load`, `unload`, `list`, `status`, `refresh`, `set-var`, `import`, `ping`.
   - Formatted terminal output for human and JSON output (`--json`).
4. Export `pub mod ipc; pub mod runtime;` in `crates/pluvia-daemon/src/lib.rs`.

## Target Files
- `crates/pluvia-daemon/Cargo.toml` (ensure `tokio = { version = "1.40", features = ["full"] }`, `serde_json = "1.0"`, `futures = "0.3"`)
- `crates/pluvia-daemon/src/runtime.rs`
- `crates/pluvia-daemon/src/ipc.rs`
- `crates/pluvia-daemon/src/lib.rs`
- `crates/pluvia-daemon/src/main.rs`
- `crates/pluvia-cli/Cargo.toml` (add `tokio = { version = "1.40", features = ["full"] }`, `serde = { version = "1.0", features = ["derive"] }`, `serde_json = "1.0"`, `clap = { version = "4.5", features = ["derive"] }`)
- `crates/pluvia-cli/src/client.rs`
- `crates/pluvia-cli/src/main.rs`
- `crates/pluvia-daemon/tests/test_ipc.rs`

## Instructions
1. Follow TDD: create failing unit/integration tests in `crates/pluvia-daemon/tests/test_ipc.rs` covering:
   - JSON-RPC 2.0 ping and pong response.
   - `pluvia.getState` returning daemon status, uptime, and active instances.
   - `pluvia.loadSkin` and `pluvia.unloadSkin` updating skin state.
   - Method-not-found (`-32601`) and parse error (`-32700`) error handling.
   - `pluvia-cli` client connecting and executing requests over Unix domain socket.
2. Verify failure with `cargo test -p pluvia-daemon --test test_ipc`.
3. Implement `runtime.rs`, `ipc.rs`, `client.rs`, and `main.rs`.
4. Verify all tests pass with `cargo test --all`.
5. Commit with message: `feat(ipc): implement JSON-RPC 2.0 socket server and CLI controller`
6. Write completion report to `.superpowers/sdd/2026-09-18-pluvia-core-engine/task-8-report.md`.
7. Return status under 15 lines.
