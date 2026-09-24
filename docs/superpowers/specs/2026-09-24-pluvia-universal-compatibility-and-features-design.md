# Pluvia Universal Compatibility, Z-Order Layering & Audio-Visualizer Architecture

**Status:** Approved  
**Author:** DRNZY & Antigravity  
**Date:** 2026-09-24  
**Target:** Pluvia Engine (`pluvia-core`, `pluvia-daemon`, `pluvia-studio`)

---

## 1. Executive Summary

This architecture upgrades the Pluvia Rainmeter engine to achieve universal compatibility across arbitrary `.rmskin` packages on Linux. It solves core visual, behavioral, and window-management limitations observed in real-world skin deployments:

1. **Window Layering (Z-Order):** Keeps desktop widgets pinned strictly below normal application windows using Wayland layer-shell `Background`/`Bottom` layers and X11 `_NET_WM_WINDOW_TYPE_DESKTOP`.
2. **Auto-Pause on Fullscreen/Gaming:** Automatically suspends rendering and measure polling when any application enters fullscreen to ensure 0% CPU and GPU utilization.
3. **Font Registration Engine:** Dynamically registers font files from skin `@Resources/Fonts/*.ttf` / `*.otf` into Fontconfig and Pango so custom icon glyphs and typography render correctly without mojibake (`à [0089A]`).
4. **Universal String Formatting:** Implements Rainmeter standard `AutoScale`, `NumOfDecimals`, `Scale`, `Prefix`, `Postfix`, and `Percentual` attributes to prevent unbounded floating point numbers (e.g. `2.830188679... B/s`).
5. **Section Variables & Container Masking:** Implements dynamic meter geometry queries (`[MeterName:W]`, `[MeterName:H]`, `[MeterName:X]`, `[MeterName:Y]`) and Cairo clipping masks (`Container=MeterName`) required by complex animated visualizers (such as Amogus).
6. **Telemetry & Media Generalization:** Maps Windows drive letters to Linux mount points (`/`, `/home`, `/run/media/*`) and provides multi-band PipeWire audio spectrum analysis and MPRIS2 media player integration.

---

## 2. Subsystem Architecture

### 2.1 Window Layering & Desktop Placement (`pluvia-daemon`)

Widgets must never float above active application windows or steal keyboard focus.

#### Wayland Protocol (`LayerShell`)
- Set `layer` to `Layer::Background` (or `Layer::Bottom`).
- Set `exclusive_zone` to `0` so widgets do not push, pad, or resize tiled windows.
- Set `keyboard_interactivity` to `None`.
- Compute the Cairo input region from non-transparent pixels so transparent widget areas allow pointer events to pass through to the wallpaper.

#### X11 / XWayland Protocol
- Set `_NET_WM_WINDOW_TYPE` to `_NET_WM_WINDOW_TYPE_DESKTOP`.
- Set `_NET_WM_STATE` to `_NET_WM_STATE_BELOW`.
- Apply `XShapeCombineRegion` to define input passthrough regions.

---

### 2.2 Fullscreen & Gaming Auto-Pause Engine (`pluvia-daemon`)

To eliminate CPU and GPU consumption during gaming or fullscreen media playback:

1. **Detector Loop:**
   - Monitor X11/XWayland root window property `_NET_ACTIVE_WINDOW` and check for `_NET_WM_STATE_FULLSCREEN`.
   - Monitor D-Bus `org.freedesktop.ScreenSaver` / `Inhibit` signals.
2. **State Transition:**
   - When a fullscreen window is detected: Transition daemon state to `Suspended`. Halt all render frame callbacks and measure calculation worker threads.
   - When exiting fullscreen: Transition back to `Active`. Immediately trigger a single redraw and resume configured `Update` intervals.

---

### 2.3 Font Registration & Glyph Engine (`pluvia-core` & `pluvia-daemon`)

When a skin suite is loaded:
1. Scan the skin's `@Resources/Fonts/` and `#@#Fonts/` directories for `.ttf`, `.otf`, and `.woff` font files.
2. Register discovered fonts with the runtime Fontconfig configuration (`FcConfigAppFontAddFile`) and Pango font map (`pango_cairo_font_map_get_default`).
3. When parsing `FontFace` in `Meter=String` or `MeterStyle`, resolve font families accurately across system fonts and bundled skin fonts.

---

### 2.4 Universal Meter Formatting & Container Masking (`pluvia-core`)

#### Numeric & String Formatting
When evaluating a `Meter=String` bound to measures or dynamic variables:
- `AutoScale=1`: Automatically format raw byte values into human-readable units (`B`, `kB`, `MB`, `GB`, `TB`) using 1024 base.
- `AutoScale=2`: Format using 1000 base.
- `AutoScale=1k`: Format starting at kilo units.
- `NumOfDecimals=N`: Round numeric output to `N` decimal places (default 0 for integers, default 1-2 when AutoScale is active).
- `Scale=N`: Divide measure value by `N` before formatting.
- `Prefix` / `Postfix`: Prepend/append string literals.
- `Percentual=1`: Scale value to 0-100% based on measure Min/Max values.

#### Section Variables & Geometry Resolution
Support dynamic section variables during formula evaluation and string substitution:
- `[SectionName:W]`: Current rendered width of the meter.
- `[SectionName:H]`: Current rendered height of the meter.
- `[SectionName:X]`: Current X position of the meter.
- `[SectionName:Y]`: Current Y position of the meter.
- `[SectionName:MinValue]`, `[SectionName:MaxValue]`: Measure boundary values.

#### Container Masking (`Container=MeterName`)
In `render/cairo.rs`:
1. If a meter specifies `Container=ParentMeter`, identify the parent meter in the skin.
2. Render the parent meter's alpha channel to an off-screen mask surface.
3. Apply `cairo_mask_surface` / `cairo_clip` before rendering the child meter so it is strictly clipped to the container's visual outline.

---

### 2.5 System Telemetry & Audio/Media Measures (`pluvia-core`)

#### FreeDiskSpace & Drive Mapping
- Map drive letters (`C:`, `D:`, `E:`, `F:`) dynamically:
  - `C:` maps to root `/`.
  - `D:` maps to `/home`.
  - `E:`, `F:`, etc. map to mounted external drives under `/run/media/$USER/*` or `/media/*`.
- Provide `TotalSpace`, `FreeSpace`, `UsedSpace`, `DiskReadBytes`, `DiskWriteBytes`.

#### AudioLevel Multi-Band Visualizer
- Capture PipeWire / PulseAudio monitor stream in `audio_capture.rs`.
- Compute FFT spectrum across configured frequency bands (e.g. 16 to 128 bands).
- Support smoothing parameters (`Sensitivity`, `FFTAttack`, `FFTDecay`, `FreqMin`, `FreqMax`).

#### NowPlaying / Media Control
- Query D-Bus MPRIS2 interface for active media players.
- Support metadata: Title, Artist, Album, AlbumArt URI, Duration, Position.
- Support interactive Bang actions: `[!CommandMeasure NowPlaying "PlayPause"]`, `[!CommandMeasure NowPlaying "Next"]`, `[!CommandMeasure NowPlaying "Previous"]`.

---

## 3. Verification Plan

1. **Unit & Integration Tests:**
   - Test String meter formatting with `AutoScale`, `NumOfDecimals`, `Scale`, and `Percentual`.
   - Test section variable geometry resolution (`[Meter:W]`, `[Meter:H]`).
   - Test font discovery and Fontconfig registration.
   - Test container mask clipping in Cairo rendering.
2. **Real-World Skin Test Suite:**
   - Load `Amogus` widget and verify glyph visualizer calculations.
   - Load `TronMCP` (Drive, CPU, RAM, NET, Audio) and verify clean numbers and fonts without mojibake.
   - Load `Monterey` (Music, Calendar, Weather) and verify responsive scaling.
3. **Desktop Verification on Linux / GNOME Wayland:**
   - Verify widgets stay behind normal browser/terminal windows.
   - Verify auto-pause suspends rendering during fullscreen.
