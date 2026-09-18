# Pluvia: Rainmeter-Compatible Desktop Widget Engine for Linux
**Design Specification**  
*Date: 2026-09-18*  
*Status: Verified & Production-Hardened*

---

## 1. Executive Summary

Pluvia is an open-source, ultra-lightweight, native Linux desktop widget engine and ecosystem engineered to natively execute the vast catalog of existing Windows Rainmeter skins (`.rmskin` and `.ini`). 

Pluvia is architected across two decoupled tiers:
1. **Pluvia Lite (`pluvia-daemon` & `pluvia-cli`)**: A high-performance, single-binary daemon written in Rust that operates under **12 MB RAM** with $\le 0.1\%$ idle CPU. It directly ingests Rainmeter `.ini` syntax, evaluates measures against Linux system interfaces (`/proc`, D-Bus, PipeWire), dynamically binds fonts into memory, and draws transparent desktop overlay windows via Cairo and native display backends.
2. **Pluvia Studio (`pluvia-studio`)**: A modern desktop management studio and skin hub providing visual layout inspection, 1-click drag-and-drop `.rmskin` installation, live coordinate adjustments, color pickers, and community skin discovery. The Studio connects to the daemon over a low-latency JSON-RPC 2.0 UNIX socket and unloads completely from memory when closed.

---

## 2. Architectural Components

```
                     ┌───────────────────────────┐
                     │   Pluvia Studio (GUI)     │
                     │  (Vue / TypeScript / Rust)│
                     └─────────────┬─────────────┘
                                   │
                     ┌─────────────┴─────────────┐
                     │    Pluvia CLI (Lite)      │
                     │   (Terminal Controller)   │
                     └─────────────┬─────────────┘
                                   │ JSON-RPC 2.0 (Request / Response + Pub/Sub)
                                   ▼ UNIX Socket: /run/user/<uid>/pluvia.sock
   ┌─────────────────────────────────────────────────────────────────────────┐
   │                           pluvia-daemon (Rust)                          │
   │                                                                         │
   │  ┌──────────────────────┐  ┌─────────────────────┐  ┌────────────────┐  │
   │  │   Skin Parser (.ini) │  │   Telemetry Engine  │  │ Anti Zip-Slip  │  │
   │  │  • Charset Transcode │  │  • /proc/stat (CPU) │  │  Extractor     │  │
   │  │    (UTF-16LE / CP1252│  │  • /proc/meminfo    │  │  • No Symlinks │  │
   │  │  • Section Lexer     │  │  • MPRIS D-Bus      │  │  • Component   │  │
   │  │  • Case-Insensitive  │  │  • PipeWire Audio   │  │    Inspection  │  │
   │  │    VFS Path Resolver │  │                     │  │  • Canonical   │  │
   │  └──────────┬───────────┘  └──────────┬──────────┘  └────────────────┘  │
   │             │                         │                                 │
   │             ▼                         ▼                                 │
   │  ┌───────────────────────────────────────────────┐  ┌────────────────┐  │
   │  │            Measure & Plugin Engine            │  │ Jailed Lua 5.1 │  │
   │  │  • Native Measures (Time, CPU, Mem, Disk, Net)│◄─┤ VFS Sandbox    │  │
   │  │  • Native C++ DLL Emulators:                  │  │ (Scoped I/O)   │  │
   │  │    ActionTimer | AudioLevel | Win7Audio       │  └────────────────┘  │
   │  │    WebParser   | Process                      │                      │
   │  └──────────────────────┬────────────────────────┘                      │
   │                         │                                               │
   │                         ▼                                               │
   │  ┌───────────────────────────────────────────────┐                      │
   │  │       PangoCairo + HarfBuzz + Fontconfig      │                      │
   │  │  • Dynamic @Resources Fontconfig Registration │                      │
   │  │  • OpenType Ligatures & Complex Text Shaping  │                      │
   │  │  • Dynamic Alpha Hit-Test Input Masks         │                      │
   │  └──────────────────────┬────────────────────────┘                      │
   │                         │                                               │
   │                         ▼                                               │
   │  ┌───────────────────────────────────────────────┐                      │
   │  │          Multi-Backend Display Layer          │                      │
   │  │  • Wayland: wlr-layer-shell (anchors/margins) │                      │
   │  │  • GNOME Wayland: pluvia-shell GNOME Extension│                      │
   │  │  • X11: _NET_WM_WINDOW_TYPE_DESKTOP / xcb     │                      │
   │  └───────────────────────────────────────────────┘                      │
   └─────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Directory Layout & XDG Compliance

Pluvia strictly conforms to the XDG Base Directory specification:

| Role | Path | Purpose |
| :--- | :--- | :--- |
| **Skins Storage** | `~/.local/share/pluvia/Skins/` | Installed skins (e.g. `Mond/Clock/Clock.ini`). Mirrors Windows `Documents/Rainmeter/Skins/`. |
| **Active Layout & Settings** | `~/.config/pluvia/` | `config.json` persisting active loaded skins, monitor anchors, lock states, click-through, and variables. |
| **Ephemeral Cache** | `~/.cache/pluvia/` | Downloaded `.rmskin` staging, temporary web/weather feeds, album art thumbnails, and parsed font caches. |
| **Runtime Socket** | `/run/user/<uid>/pluvia.sock` | Ephemeral UNIX domain socket for bidirectional IPC. |

---

## 4. Rainmeter Compatibility & Hardened Subsystems

### 4.1 Automated Charset Detection & Transcoding (UTF-16 LE & Windows-1252)
Windows Rainmeter natively writes configurations in UTF-16 LE with BOM (`0xFF 0xFE`), while legacy skins frequently use Windows-1252 (CP1252 ANSI). Directly parsing bytes as UTF-8 leads to invalid byte errors.
- **BOM Inspection**: Probes initial bytes for UTF-16 LE (`0xFF 0xFE`), UTF-16 BE (`0xFE 0xFF`), and UTF-8 BOM (`0xEF 0xBB 0xBF`).
- **Transcoder (`encoding_rs`)**: Automatically decodes UTF-16 LE and CP1252 into normalized UTF-8 in memory prior to lexing.

### 4.2 Virtual Path & Case-Insensitive VFS Resolver
Windows filesystems (NTFS) are case-insensitive and use `\` separators. Linux filesystems (`ext4`, `btrfs`) are case-sensitive and use `/`. Skin authors frequently mix cases (e.g. referencing `#@#images\clock_face.png` when the disk file is `@Resources/Images/Clock_Face.png`).
- **Path Normalization**: Translates all Windows backslashes `\` to forward slashes `/`, normalizes consecutive slashes, and strips Windows drive prefixes (`C:`).
- **Case-Insensitive Traversal**: If an exact path lookup fails, Pluvia performs a case-insensitive directory scan by matching `entry_name.to_lowercase() == segment.to_lowercase()`.
- **In-Memory Trie**: Resolved paths are cached per skin suite for $O(1)$ subsequent lookups.

### 4.3 Native Linux Reimplementation of Windows C++ DLL Plugins
Rainmeter skins rely on compiled Windows C++ DLL plugins via `Measure=Plugin` and `Plugin=<Name>.dll`. Pluvia provides native built-in Rust implementations:
1. **`ActionTimer.dll`**:
   - Emulates Rainmeter's animation framework with an async timer event loop.
   - Executes multi-step animations, interpolation curves, dynamic variable increments, and discrete delay actions (`ActionList1=Repeat MoveUp, 16, 20`).
2. **`AudioLevel.dll`**:
   - Direct connection to PipeWire / PulseAudio recording monitors.
   - Performs low-latency Fast Fourier Transform (FFT) computations, computing frequency bands, RMS levels, and peak volume per channel for audio visualizers.
3. **`Win7AudioPlugin.dll`**:
   - Maps Windows master audio control to Linux PulseAudio/PipeWire sink APIs (volume get/set, mute toggle, device queries).
4. **`Process.dll`**:
   - Checks whether specific application processes are running by inspecting `/proc`.
5. **`WebParser.dll`**:
   - Asynchronous background HTTP/HTTPS fetching with RegEx extraction, XML/JSON parsing, and cache management in `~/.cache/pluvia/`.

### 4.4 Advanced Text Rendering: Pango, HarfBuzz & Fontconfig
Relying on raw FreeType rasterization cannot resolve system font family names (e.g. `FontFace=Segoe UI` or `Arial`) and lacks OpenType font shaping:
- **Fontconfig Integration**: Registers local `@Resources/Fonts/` into the application's in-memory Fontconfig configuration (`FcConfigAppFontAddFile`). Resolves Windows standard fonts to Linux system equivalents (e.g., `Segoe UI` $\rightarrow$ system font fallback).
- **Pango & HarfBuzz Layout**: Uses `pangocairo` with HarfBuzz for complex OpenType ligatures, kerning, bidirectional text, and font fallback chains (ensuring CJK, Arabic, and emojis render without broken boxes).

### 4.5 Dynamic Alpha Hit-Test Masks & Input Pass-Through
Rainmeter skins frequently define wide transparent window bounds with small floating meters. By default, Wayland and X11 treat the entire window bounding rectangle as an input barrier, creating invisible dead zones that block desktop clicks.
- **Alpha Mask Calculation**: On every frame redraw, Pluvia computes the bounding polygons of non-transparent, interactive meters (`LeftMouseUpAction`, buttons).
- **Wayland Input Regions**: Submits the interactive bounding boxes via `wl_surface.set_input_region()`. Completely transparent areas are excluded, allowing clicks to pass through directly to the desktop wallpaper and icons.
- **X11 Input Shapes**: Employs `XFixesSetWindowShapeRegion` / X11 Shape extension with `ShapeInput` to punch out transparent regions from the mouse hit-test grid.

### 4.6 Jailed, Scoped Lua 5.1 Sandbox
Multi-file Rainmeter Lua modules rely on `require()`, `io.open()`, and `loadfile()` to parse feeds and configurations. Pluvia implements a **chrooted virtual filesystem jail**:
- **Scoped Read Access**: `require()`, `loadfile()`, and `io.open(path, "r")` are strictly confined to:
  - The skin's directory: `~/.local/share/pluvia/Skins/<SkinSuite>/...`
  - The skin's cache directory: `~/.cache/pluvia/<SkinSuite>/...`
- **Scoped Write Access**: `io.open(path, "w")` is permitted *only* within `~/.cache/pluvia/<SkinSuite>/`.
- **Sandbox Boundary Enforcement**: Any attempt to traverse out of the sandbox (e.g. `../../../../etc/passwd` or `/home/user/.ssh`) returns an explicit `PermissionDenied` error.
- **Dangerous Globals Stripped**: `os.execute`, `os.remove`, `os.rename`, `io.popen`, and `package.loadlib` are eradicated to block arbitrary process execution.

---

## 5. Security & Extraction: Anti Zip-Slip & Symlink Immunity

### 5.1 Absolute Symlink & Zip-Slip Protection
In zip archives, Unix file permissions can encode symlinks. If a malicious `.rmskin` extracts a symlink targeting `~/.ssh` or `/etc`, subsequent entries written through that symlink bypass component validation:
- **Symlink Prohibition**: Any archive entry possessing Unix symlink attributes (`entry.unix_mode() & 0o170000 == 0o120000`) or hardlink attributes is rejected with `SecurityError::SymlinkForbidden`.
- **Component Validation**:
  ```rust
  let enclosed = entry.enclosed_name().ok_or(SecurityError::ZipSlipDetected)?;
  if enclosed.components().any(|c| matches!(c, Component::ParentDir | Component::Prefix(_) | Component::RootDir)) {
      return Err(SecurityError::ZipSlipDetected);
  }
  let target_path = destination_root.join(enclosed);
  ```
- **Target Verification**: Prior to writing, verifies `target_path` is strictly prefixed by `canonicalize(destination_root)`.

---

## 6. Multi-Backend Display Layer (Including GNOME Wayland)

Mutter (GNOME) does not implement `wlr-layer-shell` and treats XWayland surfaces as standard client windows, causing focus stealing and workspace breakage. Pluvia implements a strict multi-backend architecture:

1. **Wayland wlroots & KDE Plasma (`wlr-layer-shell`)**:
   - Targets `ZWLR_LAYER_SHELL_V1_LAYER_BOTTOM` or `BACKGROUND`.
   - Uses layer-shell anchors (Top, Bottom, Left, Right) and margins for resolution-independent positioning.
2. **GNOME Wayland: Dedicated Shell Extension (`pluvia-shell@pluvia.org`)**:
   - A required, lightweight GNOME Shell extension for GNOME Wayland sessions.
   - Attaches Pluvia surface actors directly to `global.window_group` beneath all normal application windows, guaranteeing they never steal focus, remain pinned during workspace transitions, and survive "Show Desktop" interactions.
3. **Pure X11**:
   - Direct XCB/X11 desktop window (`_NET_WM_WINDOW_TYPE_DESKTOP`, `_NET_WM_STATE_BELOW`, `_NET_WM_STATE_STICKY`).

---

## 7. IPC Protocol (JSON-RPC 2.0 with Pub/Sub)

Communication over `/run/user/<uid>/pluvia.sock` follows strict JSON-RPC 2.0:
- **Requests & Methods**:
  - `pluvia.loadSkin { path: "Mond/Clock/Clock.ini", anchor: "TopCenter", margin_y: 100 }`
  - `pluvia.unloadSkin { id: "Mond/Clock" }`
  - `pluvia.listSkins {}`
  - `pluvia.getActiveInstances {}`
  - `pluvia.setVariable { skin_id: "Mond/Clock", key: "Color1", value: "255,255,255" }`
  - `pluvia.importPackage { file_path: "/path/to/skin.rmskin" }`
- **Pub/Sub Subscriptions**:
  - Clients can subscribe to `notify.skinLoaded`, `notify.skinUnloaded`, `notify.meterUpdated`, and `notify.packageExtractProgress`.

---

## 8. Implementation Milestones

### Phase 1: Core Engine & Parser (`pluvia-daemon`)
- Rust workspace setup (`pluvia-core`, `pluvia-daemon`, `pluvia-cli`).
- Charset transcoder (UTF-16 LE, CP1252) & case-insensitive VFS resolver.
- PangoCairo + HarfBuzz + Fontconfig text rendering pipeline.
- System measures (`Time`, `CPU`, `Memory`, `Disk`, `NowPlaying`).
- Native plugin emulators (`ActionTimer`, `AudioLevel`, `Win7Audio`, `Process`, `WebParser`).
- Jailed Lua 5.1 VFS sandbox.
- Cairo 2D renderer for all core meters with alpha hit-test masks.
- Display backend abstraction (`wlr-layer-shell`, `pluvia-shell` extension, X11).

### Phase 2: Package Management & IPC
- Hardened Anti Zip-Slip & symlink-immune `.rmskin` extractor.
- JSON-RPC 2.0 UNIX domain socket server with event streaming.
- `pluvia-cli` terminal client.
- Bundled default skins: Mond Clock, System Telemetry, Media Player.

### Phase 3: Pluvia Studio (Full GUI Manager)
- Desktop management application.
- Skin library tree and active instances panel.
- Visual positioning, monitor picker, and live variable editor.
- Drag-and-drop `.rmskin` installer.
