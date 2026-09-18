# Task 8 Report: UNIX Domain Socket JSON-RPC 2.0 Server & CLI

## Summary
Implemented the daemon runtime orchestration engine (`SkinRuntime`), the asynchronous UNIX domain socket JSON-RPC 2.0 server (`IpcServer`), and the command-line controller (`pluvia-cli` / `PluviaClient`). The daemon manages loaded skin instances, schedules update tickers, polls telemetry measures, renders meters to Cairo surfaces, submits `AlphaHitMask` click-through masks to desktop surfaces, and clears damage rects per tick to prevent unbounded memory growth. The IPC server implements standard JSON-RPC 2.0 with pub/sub event broadcasting, error handling, package import, and full CLI parity.

## Implemented Components

1. **Skin Runtime Engine (`crates/pluvia-daemon/src/runtime.rs`)**:
   - `SkinRuntime`:
     - Manages active skin instances (`SkinInstance`) mapped by ID/name.
     - Loads skins from `.ini` configuration files (`load_skin`) with measure instantiation via `create_measure` and dynamic surface bounds calculation.
     - Performs initial and continuous rendering cycles via `MeterRenderer::render_to_surface`.
     - Submits rendered pixel buffers and `AlphaHitMask` click-through regions to `DesktopSurface`.
     - Calls `surface.clear_damage()` each tick to prevent unbounded frame damage accumulation.
     - Dynamically manages skin variables (`set_variable`), reloading configurations (`refresh_skin`), and batch operations (`refresh_all`, `tick_all`).
     - Exposes daemon status, active instances, and uptime in `get_state`.
   - `start_background_ticker`:
     - Spawns background tick intervals invoking `tick_all` with graceful shutdown signaling.

2. **JSON-RPC 2.0 IPC Server (`crates/pluvia-daemon/src/ipc.rs`)**:
   - Asynchronous Tokio UNIX domain socket server implementing JSON-RPC 2.0 specification.
   - Socket path discovery via `default_socket_path()`:
     - Respects `PLUVIA_SOCKET` environment override.
     - Falls back to `$XDG_RUNTIME_DIR/pluvia.sock`, `/run/user/<uid>/pluvia.sock`, `~/.cache/pluvia/pluvia.sock`, or `/tmp/pluvia.sock`.
   - Implemented JSON-RPC methods:
     - `pluvia.ping`: Returns `"pong"`.
     - `pluvia.loadSkin`: Loads skin from specified path, returns `SkinInfo`, broadcasts `notify.skinLoaded`.
     - `pluvia.unloadSkin`: Unloads skin instance, destroys surface, returns confirmation, broadcasts `notify.skinUnloaded`.
     - `pluvia.listSkins`: Enumerates all active skins with IDs, paths, bounds, and measure/meter counts.
     - `pluvia.getState`: Queries daemon health, uptime, backend, and loaded skin instances.
     - `pluvia.refreshSkin` & `pluvia.refreshAll`: Triggers re-parsing and re-rendering of active skins.
     - `pluvia.setVariable`: Updates skin variables at runtime and triggers re-render.
     - `pluvia.importPackage`: Validates and extracts `.rmskin` packages using `extract_rmskin_package`, returning extraction reports and broadcasting `notify.packageExtractProgress`.
   - Error handling:
     - Parse error (`-32700`)
     - Invalid request (`-32600`)
     - Method not found (`-32601`)
     - Invalid params (`-32602`)
     - Server error (`-32000`)
   - Pub/Sub notifications:
     - Broadcasts asynchronously to all connected client streams using Tokio `broadcast::channel`.

3. **Pluvia CLI Client Library (`crates/pluvia-cli/src/client.rs` & `src/lib.rs`)**:
   - `PluviaClient`:
     - Connects over UNIX domain socket and issues typed JSON-RPC 2.0 requests.
     - Supports asynchronous notifications while isolating responses by unique request IDs.
     - Provides ergonomic async methods for all daemon commands.

4. **Command-Line Controller (`crates/pluvia-cli/src/main.rs`)**:
   - Built with `clap` derive parser supporting:
     - `ping`: Verifies connection to running daemon.
     - `load <path>`: Loads a skin `.ini` file.
     - `unload <id>`: Unloads active skin.
     - `list`: Lists active skins in tabular format.
     - `status`: Displays daemon status, uptime, backend, and skin count.
     - `refresh [id]`: Refreshes individual or all skins.
     - `set-var <id> <key> <value>`: Updates skin variables on the fly.
     - `import <archive_path> [--dest <path>]`: Imports `.rmskin` packages.
   - Global flags:
     - `--socket <path>`: Custom socket location.
     - `--json`: Formats all output as JSON for programmatic consumption.

5. **Daemon Main Entrypoint (`crates/pluvia-daemon/src/main.rs`)**:
   - Parses CLI arguments (`--socket`, `--mock`, `--help`).
   - Resolves display backend via `BackendType::detect()`.
   - Initializes `SkinRuntime`, launches background ticker task, binds `IpcServer`, and handles graceful termination on Ctrl+C (`SIGINT`).

6. **Desktop Surface Damage Clearing**:
   - Extended `DesktopSurface` trait with `fn clear_damage(&mut self)`.
   - Implemented `clear_damage` across `MockDesktopSurface`, `LayerShellSurface`, `X11Surface`, and `GnomeBridgeSurface`.
   - Implemented `Send` and `Sync` for `MeterRenderer` to support concurrent runtime access across Tokio async tasks.

## Tests & Verification

- **TDD RED Phase**:
  - Authored comprehensive unit and integration tests in `crates/pluvia-daemon/tests/test_ipc.rs`.
  - Confirmed all 8 tests failed predictably with `todo!("TDD...")` panic.
- **TDD GREEN Phase**:
  - Implemented `runtime.rs`, `ipc.rs`, `client.rs`, and CLI entrypoints.
  - Re-ran `cargo test -p pluvia-daemon --test test_ipc`:
    1. `test_ipc_ping_pong`: verified JSON-RPC 2.0 ping/pong protocol exchange.
    2. `test_ipc_get_state`: verified daemon status, uptime, backend, and skin instance reporting.
    3. `test_ipc_load_and_unload_skin`: verified loading, listing, and unloading skin instances.
    4. `test_ipc_error_handling`: verified error codes `-32700` (Parse error) and `-32601` (Method not found).
    5. `test_cli_client_integration`: verified `PluviaClient` connecting, pinging, getting state, loading, setting variables, refreshing, and unloading over socket.
    6. `test_ipc_pubsub_notifications`: verified real-time `notify.skinLoaded` and `notify.skinUnloaded` delivery to connected subscribers.
    7. `test_ipc_package_import`: verified extracting `.rmskin` packages over IPC.
    8. `test_runtime_tick_clears_damage`: verified damage history is wiped per tick to avoid memory leaks.
- **Workspace Verification**:
  - Ran `cargo test --all`: 80 tests passed across the workspace (65 `pluvia-core`, 15 `pluvia-daemon`) with 0 failures.
  - Ran `cargo check --all --tests`: 0 warnings, pristine build.
  - Tested CLI binary commands `target/debug/pluvia-cli --help` and `target/debug/pluvia-daemon --help`.
