# Task 7 Brief: Display Layer & Window Backend

## Overview
Implement the multi-backend display abstraction and window positioning engine in `crates/pluvia-daemon`:
1. `crates/pluvia-daemon/src/display/mod.rs`:
   - `DesktopSurface` trait defining surface lifecycle, bounds, rendering, visibility, and alpha hit-test mask updates.
   - `Anchor` enum (`TopLeft`, `TopCenter`, `TopRight`, `Center`, `BottomLeft`, `BottomCenter`, `BottomRight`, `Absolute`) and coordinate calculations for single and multi-monitor layouts.
   - Display backend detector (`BackendType::detect()`) selecting `WlrLayerShell`, `GnomeWayland`, or `X11`.
2. `crates/pluvia-daemon/src/display/layer_shell.rs`:
   - Wayland `wlr-layer-shell` backend (targeting layer `BOTTOM`/`BACKGROUND`, setting edge anchors and pixel margins, submitting `AlphaHitMask` rects to `wl_surface.set_input_region` for click-through).
3. `crates/pluvia-daemon/src/display/x11.rs`:
   - X11 backend setting `_NET_WM_WINDOW_TYPE_DESKTOP`, `_NET_WM_STATE_BELOW`, `_NET_WM_STATE_STICKY`, `_NET_WM_DESKTOP = 0xFFFFFFFF`, and using XFixes/ShapeInput with `AlphaHitMask` rects for click-through.
4. `crates/pluvia-daemon/src/display/gnome_bridge.rs`:
   - GNOME Wayland integration: D-Bus bridge to `pluvia-shell@pluvia.org` GNOME Shell extension with fallback to XWayland desktop window.
5. `crates/pluvia-daemon/src/display/mock.rs`:
   - Mock desktop surface for hermetic headless testing.
6. Export `pub mod display;` in `crates/pluvia-daemon/src/lib.rs`.

## Target Files
- `crates/pluvia-daemon/Cargo.toml` (depend on `pluvia-core = { path = "../pluvia-core" }`, `cairo-rs = "0.20"`, `tokio.workspace = true`, `serde.workspace = true`, `thiserror.workspace = true`, `anyhow.workspace = true`)
- `crates/pluvia-daemon/src/lib.rs`
- `crates/pluvia-daemon/src/display/mod.rs`
- `crates/pluvia-daemon/src/display/layer_shell.rs`
- `crates/pluvia-daemon/src/display/x11.rs`
- `crates/pluvia-daemon/src/display/gnome_bridge.rs`
- `crates/pluvia-daemon/src/display/mock.rs`
- `crates/pluvia-daemon/tests/test_display_bounds.rs`

## Instructions
1. Follow TDD: create failing unit tests in `crates/pluvia-daemon/tests/test_display_bounds.rs` covering:
   - Coordinate resolution for all `Anchor` types across various screen resolutions (1920x1080, 2560x1440, 5120x1440).
   - Multi-monitor coordinate translation with monitor offsets.
   - `MockDesktopSurface` lifecycle and `AlphaHitMask` disjoint rectangle submission.
   - Environment detection logic for Wayland / GNOME / X11.
2. Verify failure with `cargo test -p pluvia-daemon --test test_display_bounds`.
3. Implement `mod.rs`, `layer_shell.rs`, `x11.rs`, `gnome_bridge.rs`, `mock.rs`.
4. Verify all tests pass with `cargo test --all`.
5. Commit with message: `feat(daemon): implement multi-backend display abstraction with layer-shell, GNOME bridge, and X11`
6. Write completion report to `.superpowers/sdd/2026-09-18-pluvia-core-engine/task-7-report.md`.
7. Return status under 15 lines.
