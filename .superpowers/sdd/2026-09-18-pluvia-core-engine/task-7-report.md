# Task 7 Report: Display Layer & Window Backend

## Summary
Implemented the multi-backend display abstraction and window positioning engine in `crates/pluvia-daemon`. The architecture abstracts desktop window surfaces across Wayland (`wlr-layer-shell`), GNOME Wayland (`pluvia-shell@pluvia.org` D-Bus bridge with XWayland fallback), and X11 (EWMH desktop atoms and XShape/XFixes click-through masks). In addition, a coordinate calculation engine resolves single and multi-monitor layout positions across all 8 `Anchor` placements, and a hermetic `MockDesktopSurface` enables headless testing and verification.

## Implemented Components

1. **Common Display Abstractions & Coordinate Engine (`crates/pluvia-daemon/src/display/mod.rs`)**:
   - `DesktopSurface` trait:
     - Defines surface lifecycle (`bounds`, `set_bounds`, `is_visible`, `set_visible`, `destroy`, `is_destroyed`).
     - Content rendering & input masking (`update_surface`, `update_hit_mask`).
     - Backend identification (`backend_type`).
   - `Anchor` enum:
     - 8 positioning modes: `TopLeft`, `TopCenter`, `TopRight`, `Center`, `BottomLeft`, `BottomCenter`, `BottomRight`, `Absolute`.
     - Inward edge margin math resolving exact top-left canvas coordinates and bounding boxes across standard (1080p, 1440p) and ultrawide (5120x1440) displays.
   - `ScreenGeometry` & `DisplayLayout`:
     - Multi-monitor tracking with monitor names, offsets, dimensions, and scale factors.
     - Coordinate point lookup (`find_monitor_at`) and monitor-to-monitor relative coordinate translation (`translate_relative`, `translate_point`).
   - `BackendType::detect()`:
     - Detects desktop environment via `WAYLAND_DISPLAY`, `XDG_SESSION_TYPE`, `XDG_CURRENT_DESKTOP`, `XDG_SESSION_DESKTOP`, `GNOME_DESKTOP_SESSION_ID`, and `DISPLAY`.
     - Selects `GnomeWayland` for GNOME on Wayland, `WlrLayerShell` for wlroots/standard Wayland (Sway, Hyprland), and `X11` for Xorg.
     - Supports explicit test and operational override via `PLUVIA_BACKEND` (supporting `mock`, `layer-shell`, `gnome`, `x11`).

2. **Wayland Layer-Shell Backend (`crates/pluvia-daemon/src/display/layer_shell.rs`)**:
   - `LayerShellSurface`:
     - Targets Wayland layer-shell protocol (`zwlr_layer_shell_v1`) targeting `Layer::Bottom` (above wallpaper, below windows) or `Layer::Background`.
     - Translates `Anchor` into `LayerShellAnchor` bitflags (`TOP`, `BOTTOM`, `LEFT`, `RIGHT`).
     - Computes layer-shell directional pixel margins `(top, right, bottom, left)`.
     - Decomposes `AlphaHitMask` into disjoint rectangular bounding boxes submittable to `wl_surface.set_input_region` for 100% click-through outside active meters.
     - Tracks buffer damage rects across updates.

3. **X11 EWMH & ShapeInput Backend (`crates/pluvia-daemon/src/display/x11.rs`)**:
   - `X11Surface`:
     - Configures EWMH window hints:
       - `_NET_WM_WINDOW_TYPE = _NET_WM_WINDOW_TYPE_DESKTOP`
       - `_NET_WM_STATE = _NET_WM_STATE_BELOW | _NET_WM_STATE_STICKY`
       - `_NET_WM_DESKTOP = 0xFFFFFFFF` (visible across all virtual desktops)
     - Translates `AlphaHitMask` into disjoint rectangles submittable to XFixes / XShape `ShapeInput` region for seamless click-through.
     - Manages damage tracking, visibility, and bounds.

4. **GNOME Wayland Bridge Backend (`crates/pluvia-daemon/src/display/gnome_bridge.rs`)**:
   - `GnomeBridgeSurface`:
     - Provides two operating modes via `GnomeBridgeMode`:
       - `Extension`: D-Bus bridge communicating with `pluvia-shell@pluvia.org` extension under `org.pluvia.Shell`.
       - `XWaylandFallback`: Fallback desktop window backed by an internal `X11Surface` when the GNOME extension is unavailable.
     - Seamlessly routes surface updates, bounds changes, and hit-mask rect updates to the active mode.

5. **Hermetic Mock Backend (`crates/pluvia-daemon/src/display/mock.rs`)**:
   - `MockDesktopSurface`:
     - Implements `DesktopSurface` for deterministic headless and CI testing.
     - Records damage history, last surface dimensions, update counts, and submitted disjoint hit mask rectangles.

6. **Public API & Library Exposure (`crates/pluvia-daemon/src/lib.rs`)**:
   - Exposed `pub mod display;`
   - Re-exported core display types: `Anchor`, `BackendType`, `DesktopSurface`, `DisplayError`, `DisplayLayout`, `ScreenGeometry`, `SurfaceBounds`.

## Tests & Verification

- **TDD RED Phase**:
  - Authored unit and integration tests in `crates/pluvia-daemon/tests/test_display_bounds.rs`.
  - Verified compilation failure (`cannot find module or crate pluvia_daemon`) via `cargo test -p pluvia-daemon --test test_display_bounds`.
- **TDD GREEN Phase**:
  - Implemented `mod.rs`, `layer_shell.rs`, `x11.rs`, `gnome_bridge.rs`, and `mock.rs`.
  - Added dependencies `pluvia-core`, `cairo-rs`, `tokio`, `serde`, `thiserror`, `anyhow` to `crates/pluvia-daemon/Cargo.toml`.
  - Ran `cargo test -p pluvia-daemon --test test_display_bounds`:
    1. `test_anchor_coordinate_resolution_standard_screens`: verified all 8 anchors across 1920x1080, 2560x1440, and 5120x1440 resolutions with and without margins.
    2. `test_multi_monitor_coordinate_translation`: verified multi-monitor layouts, monitor lookup, and coordinate translation across display offsets.
    3. `test_mock_desktop_surface_lifecycle_and_hit_mask`: verified mock surface lifecycle, bound changes, visibility, and disjoint hit-mask rectangle extraction.
    4. `test_backend_detection_environment_logic`: verified Wayland, GNOME Wayland, X11, and `PLUVIA_BACKEND` overrides.
    5. `test_layer_shell_backend_configuration`: verified layer-shell anchors, margins, layer setting, and input regions.
    6. `test_x11_backend_window_hints_and_shape`: verified `_NET_WM_WINDOW_TYPE_DESKTOP`, `below`, `sticky`, `0xFFFFFFFF`, and ShapeInput rects.
    7. `test_gnome_bridge_extension_and_fallback`: verified D-Bus extension bridge interface and fallback to XWayland.
- **Workspace Verification**:
  - Ran `cargo test --all`: 72 tests passed across the workspace (65 in `pluvia-core`, 7 in `pluvia-daemon`) with 0 failures.
  - Ran `cargo check --all --tests`: 0 warnings, clean build.

## Git Commit
- `7d64dc3`: `feat(daemon): implement multi-backend display abstraction with layer-shell, GNOME bridge, and X11`
