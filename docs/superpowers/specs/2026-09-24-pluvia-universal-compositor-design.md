# Pluvia Universal Live Desktop Compositor & Rendering Engine
**Design Specification**  
*Date: 2026-09-24*  
*Status: Ready for Review*

---

## 1. Executive Summary

This specification defines the architectural design for the **Pluvia Universal Live Desktop Compositor & Multi-Backend Rendering Engine** (Phase 1).

The objective is to enable Rainmeter desktop skins (`.ini`) parsed by `pluvia-core` and evaluated by `pluvia-daemon` to be rendered as live, interactive, transparent desktop overlay widgets across **100% of Linux desktop environments** (Wayland, GNOME Shell, KDE Plasma, Hyprland, Sway, COSMIC, and X11/XWayland).

Key performance and compatibility requirements:
- **Memory Footprint:** $\le 15\text{ MB}$ RSS across all active skin surfaces.
- **CPU Overhead:** $\le 0.1\%$ idle CPU draw via event-driven damage rectangle rendering and dirty-frame throttling.
- **Layer Jailing:** Desktop widgets remain strictly anchored beneath normal application windows without appearing in taskbars, alt-tab switchers, or window docks.
- **Per-Pixel Click-Through:** Non-rendered / transparent pixels in a widget pass mouse click and scroll events through directly to the underlying desktop wallpaper or desktop folder icons.

---

## 2. Multi-Backend Surface Architecture

```
                               ┌───────────────────────────┐
                               │   Pluvia Runtime Engine   │
                               │  (Measure Loop & Damage)  │
                               └─────────────┬─────────────┘
                                             │ Frame Damage & Cairo Pixmap
                                             ▼
                               ┌───────────────────────────┐
                               │ Multi-Backend Dispatcher  │
                               │  (pluvia_daemon::display) │
                               └───────┬───────┬───────┬───┘
                                       │       │       │
              ┌────────────────────────┘       │       └────────────────────────┐
              ▼                                ▼                                ▼
   ┌───────────────────────┐       ┌───────────────────────┐       ┌───────────────────────┐
   │  Wayland Layer-Shell  │       │  GNOME Shell Bridge   │       │   X11 / XWayland      │
   │  (wlr-layer-shell-v1) │       │  (D-Bus / Window Hint)│       │  (EWMH Desktop Root)  │
   │ • Hyprland, Sway, KDE │       │ • GNOME 45, 46, 47    │       │ • XFCE, i3, bspwm     │
   │ • Layer::Bottom       │       │ • Mutated Layer Surface│      │ • _NET_WM_STATE_BELOW │
   └───────────────────────┘       └───────────────────────┘       └───────────────────────┘
```

---

## 3. Subsystem Specifications

### 3.1 Session Auto-Detection & Backend Selection
The daemon automatically inspects the active session environment at startup:
1. If `WAYLAND_DISPLAY` is present and `XDG_CURRENT_DESKTOP` does not contain `GNOME`:
   - Initialize the **Wayland `wlr-layer-shell`** client backend.
2. If `WAYLAND_DISPLAY` is present and `XDG_CURRENT_DESKTOP` contains `GNOME`:
   - Initialize the **GNOME Wayland Surface Bridge**, which configures borderless transparent windows with `gtk-layer-shell` compatibility or direct `org.gnome.Shell` background registration.
3. If `DISPLAY` is present without Wayland, or if Wayland layer initialization fails:
   - Initialize the **X11 EWMH** backend using `x11rb`.

### 3.2 Wayland `wlr-layer-shell` Protocol Implementation
- **Layer Selection:** Surfaces bind to `zwlr_layer_shell_v1::Layer::Bottom` (or `Layer::Background` when configured in `[Rainmeter]` section `AlwaysOnBottom=1`).
- **Anchors & Margins:** X/Y coordinates from `SkinConfig` translate into edge anchors (`TOP | LEFT`) and pixel margins.
- **Keyboard Interactivity:** `zwlr_layer_surface_v1::KeyboardInteractivity::None` to guarantee desktop widgets never steal keyboard focus from active terminal or editor sessions.
- **Exclusive Zone:** Set to `0` (widgets do not reserve screen edge space or push other windows).

### 3.3 X11 / EWMH Protocol Implementation
- **Window Type:** Sets `_NET_WM_WINDOW_TYPE` to `_NET_WM_WINDOW_TYPE_DESKTOP` and fallback `_NET_WM_WINDOW_TYPE_DOCK`.
- **Window States:** Atomically asserts `_NET_WM_STATE_BELOW`, `_NET_WM_STATE_STICKY`, and `_NET_WM_STATE_SKIP_TASKBAR` / `_NET_WM_STATE_SKIP_PAGER`.
- **Visual & Colormap:** 32-bit ARGB TrueColor visual with depth 32 to guarantee hardware-blended transparency against the root window.

### 3.4 Per-Pixel Alpha Hit-Testing & Input Masks
To prevent transparent widget bounding boxes from blocking desktop clicks:
1. After rendering each dirty frame via Cairo into an ARGB32 buffer, Pluvia extracts non-zero alpha pixels ($\alpha > 12$).
2. Contiguous pixel spans are merged into a minimal collection of bounding rectangles (`wl_region` on Wayland, `XFixesSetWindowShapeRegion` / `ShapeInput` on X11).
3. The computed input mask region is committed to the compositor surface alongside the buffer. Any click outside the visible meters passes through to the underlying desktop.

### 3.5 Multi-Monitor Display Geometry & Dynamic Hotplug
- Pluvia queries `wl_output` / `xrandr` screen geometries on startup and listens for resolution/monitor change events (`wl_output.done`, `XRROutputChangeNotifyEvent`).
- Coordinate specifiers (e.g. `Anchor=TopRight`, `Offset=(-20, 20)`, `Monitor=@1`) dynamically recalculate absolute surface coordinates when displays are connected, disconnected, or reoriented.

### 3.6 Persistence & Config File Schema
Active skins, coordinates, and layer preferences persist to `~/.config/pluvia/config.json`:
```json
{
  "version": 1,
  "loaded_skins": [
    {
      "id": "mond_clock",
      "path": "Mond/Clock/Clock.ini",
      "monitor": 0,
      "x": 80,
      "y": 60,
      "anchor": "TopLeft",
      "always_on_bottom": true,
      "click_through": false,
      "draggable": true,
      "active": true
    }
  ],
  "global_settings": {
    "target_fps": 60,
    "hardware_acceleration": true,
    "low_power_idle": true
  }
}
```

### 3.7 Systemd User Service Integration
Pluvia automatically generates and registers a user systemd service file at `~/.config/systemd/user/pluvia.service`:
```ini
[Unit]
Description=Pluvia Rainmeter Desktop Engine for Linux
PartOf=graphical-session.target
After=graphical-session.target

[Service]
Type=simple
ExecStart=%h/.local/bin/pluvia daemon
Restart=on-failure
RestartSec=3s
Slice=app-graphical.slice

[Install]
WantedBy=graphical-session.target
```

---

## 4. Verification & Testing Matrix

| Test Suite | Target Component | Validation Metric |
| :--- | :--- | :--- |
| `test_display_bounds` | Surface positioning & anchors | Validates coordinate calculation on single and multi-monitor layouts |
| `test_layer_shell_backend` | Wayland surface lifecycle | Verifies layer surface binding, margin updates, and damage commits |
| `test_x11_backend` | X11 surface lifecycle | Verifies ARGB 32-bit visual selection, EWMH atom application, and XFixes input shapes |
| `test_alpha_hit_mask` | Input masking | Verifies non-transparent bounding rectangle generation from Cairo surfaces |
| `test_persistence_roundtrip`| `config.json` serializer | Verifies atomic write, reload, and schema migration without data corruption |
| `e2e_live_desktop_render` | Full daemon tick | Loads Mond Clock + Monterey Calendar and verifies zero window decoration artifacts |

---

## 5. Implementation Boundaries & Dependencies

- **Dependencies:**
  - `smithay-client-toolkit` (Wayland layer-shell protocol bindings)
  - `x11rb` (Pure Rust X11 protocol client with XFixes and EWMH)
  - `cairo-rs` & `pango` (2D vector drawing & typography)
- **Module Boundaries:**
  - `pluvia-daemon/src/display/mod.rs` (Abstract `DesktopSurface` trait)
  - `pluvia-daemon/src/display/wayland.rs` (Wayland layer-shell driver)
  - `pluvia-daemon/src/display/x11.rs` (X11 / XWayland driver)
  - `pluvia-daemon/src/display/hit_mask.rs` (Alpha threshold region extraction)
  - `pluvia-daemon/src/display/service.rs` (Systemd user service generation)
