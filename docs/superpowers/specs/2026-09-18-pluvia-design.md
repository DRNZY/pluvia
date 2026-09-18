# Pluvia: Rainmeter-Compatible Desktop Widget Engine for Linux
**Design Specification**  
*Date: 2026-09-18*  
*Status: Reviewed & Hardened*

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
   │  │  • Section Lexer     │  │  • /proc/stat (CPU) │  │  Extractor     │  │
   │  │  • Variable Expander │  │  • /proc/meminfo    │  │  • Component   │  │
   │  │  • Case-Insensitive  │  │  • MPRIS D-Bus      │  │    Validation  │  │
   │  │    VFS Path Resolver │  │  • PipeWire Audio   │  │  • Canonical   │  │
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
   │  │              Cairo 2D Meter Canvas            │                      │
   │  │  String | Image | Bar | Roundline | Shape     │                      │
   │  │  • FreeType in-memory font loader (@Resources)│                      │
   │  └──────────────────────┬────────────────────────┘                      │
   │                         │                                               │
   │                         ▼                                               │
   │  ┌───────────────────────────────────────────────┐                      │
   │  │          Tri-Mode Display Abstraction         │                      │
   │  │  • Wayland: wlr-layer-shell (anchors/margins) │                      │
   │  │  • GNOME Wayland: XWayland / Shell Extension  │                      │
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

## 4. Rainmeter Compatibility & Hardened Systems

### 4.1 Virtual Path & Case-Insensitive VFS Resolver
Windows filesystems (NTFS) are case-insensitive and utilize backslash separators (`\`). Linux filesystems (`ext4`, `btrfs`, `zfs`) are case-sensitive and utilize forward slashes (`/`). Windows skin authors routinely mix cases (e.g. referencing `#@#images\clock_face.png` when the disk file is `@Resources/Images/Clock_Face.png`).

To eliminate missing asset crashes:
- **Path Normalization**: Translates all Windows backslashes `\` to forward slashes `/`, normalizes consecutive slashes, and strips Windows drive prefixes (e.g., `C:`).
- **Case-Insensitive Directory Traversal**: When resolving an asset path, if an exact case match fails, Pluvia performs a case-insensitive lookup across directories by matching `entry_name.to_lowercase() == segment.to_lowercase()`.
- **In-Memory Path Trie**: Results of resolved case mappings are cached in memory per skin suite to ensure zero runtime performance penalty after initial load.

### 4.2 Native Linux Reimplementation of Windows C++ DLL Plugins
A large portion of popular Rainmeter skins rely on compiled Windows C++ DLL plugins via `Measure=Plugin` and `Plugin=<Name>.dll`. Pluvia provides native built-in Rust implementations of the standard Rainmeter plugin suite:

1. **`ActionTimer.dll`**:
   - Emulates Rainmeter's animation framework.
   - Executes multi-step animations, interpolation curves, dynamic variable increments, and discrete delay actions (`ActionList1=Repeat MoveUp, 16, 20`).
2. **`AudioLevel.dll`**:
   - Emulates audio visualization and spectrum analyzers.
   - Connects directly to PipeWire / PulseAudio recording monitors.
   - Performs low-latency Fast Fourier Transform (FFT) computations, computing frequency bands, RMS levels, and peak volume per channel for audio visualizer bars.
3. **`Win7AudioPlugin.dll`**:
   - Maps Windows master audio control to Linux PulseAudio/PipeWire sink APIs (get/set volume, mute toggle, default output device name).
4. **`Process.dll`**:
   - Checks whether specific application processes (e.g. `cadence`, `spotify`, `discord`) are active by inspecting `/proc`.
5. **`WebParser.dll`**:
   - Provides asynchronous background HTTP/HTTPS fetching with RegEx extraction, XML/JSON parsing, and cache management in `~/.cache/pluvia/`.

### 4.3 Jailed, Scoped Lua 5.1 Sandbox
Multi-file Rainmeter Lua modules rely on `require()`, `io.open()`, and `loadfile()` to parse weather data, calendar feeds, and complex configs. Rather than an outright ban, Pluvia enforces a **chrooted virtual filesystem jail**:
- **Scoped Read Access**: `require()`, `loadfile()`, and `io.open(path, "r")` are strictly confined to:
  - The skin's directory: `~/.local/share/pluvia/Skins/<SkinSuite>/...`
  - The skin's cache directory: `~/.cache/pluvia/<SkinSuite>/...`
- **Scoped Write Access**: `io.open(path, "w")` is permitted *only* within `~/.cache/pluvia/<SkinSuite>/`.
- **Sandbox Boundary Enforcement**: Any attempt to traverse out of the sandbox (e.g. `../../../../etc/passwd` or `/home/user/.ssh`) returns an explicit `PermissionDenied` error.
- **Dangerous Globals Stripped**: `os.execute`, `os.remove`, `os.rename`, `io.popen`, and `package.loadlib` are eradicated to block arbitrary process execution.

### 4.4 Measures (Data Inputs)
- **`Measure=Time`**: Formats local date and time using Windows/C `strftime` format strings (`%A`, `%d`, `%B`, `%Y`, `%H`, `%M`, `%S`).
- **`Measure=CPU`**: Direct zero-allocation delta parsing of `/proc/stat` across all cores or specific core indexes.
- **`Measure=Memory`**: Reads `/proc/meminfo` to calculate `Total`, `Used`, `Free`, and percentage.
- **`Measure=FreeDiskSpace` / `Drive`**: Invokes `statvfs` on target mount points (`/`, `/home`, etc.).
- **`Measure=NetIn` / `NetOut`**: Delta bandwidth tracking via `/proc/net/dev`.
- **`Measure=NowPlaying` / `WebNowPlaying`**: Queries D-Bus `org.mpris.MediaPlayer2.*` for real-time track metadata (Artist, Title, Album, AlbumArt URI, PlaybackStatus, Position, Duration).
- **`Measure=Calc`**: Formula evaluation supporting math and logical operators.
- **`Measure=Uptime`**: Reads `/proc/uptime`.

### 4.5 Meters (Visual Primitives)
- **`Meter=String`**: Cairo + FreeType sub-pixel text rendering, dynamic in-memory font mounting from `@Resources/Fonts/`, text transforms, and alignment.
- **`Meter=Image`**: Cairo image surface supporting PNG, JPEG, SVG, WebP with scaling, aspect-ratio preservation, and `ImageTint`.
- **`Meter=Bar`**: Horizontal/vertical progress bars bound to measure values.
- **`Meter=Roundline`**: Circular/radial progress arcs, clock hands, and analog dials.
- **`Meter=Histogram`**: Historical rolling area charts and line graphs.
- **`Meter=Shape`**: Vector shapes (`Rectangle`, `RoundRectangle`, `Ellipse`, `Path`) with gradients.

---

## 5. Security & Extraction

### 5.1 True Anti Zip-Slip Package Extractor
To securely unpack `.rmskin` packages:
- **Zip-Slip Attack Surface**: In Rust, `Path::new("/dir").join("../etc/passwd")` lexically retains parent directory traversals without canonicalization, rendering raw prefix checks useless.
- **Component-Level Inspection**:
  ```rust
  // Safe extraction verification
  for entry in archive.entries() {
      let enclosed = match entry.enclosed_name() {
          Some(path) => path,
          None => return Err(SecurityError::ZipSlipDetected),
      };
      if enclosed.components().any(|c| matches!(c, Component::ParentDir | Component::Prefix(_) | Component::RootDir)) {
          return Err(SecurityError::ZipSlipDetected);
      }
      let target_path = destination_root.join(enclosed);
      // Ensure target path is cleanly within destination_root
  }
  ```
- Unpacking runs in an isolated asynchronous thread, reporting progress notifications over JSON-RPC.

---

## 6. Tri-Mode Display Layer (Including GNOME Wayland)

Upstream GNOME Mutter intentionally does not support `wlr-layer-shell`. Pluvia implements a tri-mode compositor abstraction to ensure seamless operation on all Linux desktop environments:

1. **Wayland wlroots & KDE Plasma (`wlr-layer-shell`)**:
   - Targets `ZWLR_LAYER_SHELL_V1_LAYER_BOTTOM` or `BACKGROUND`.
   - Uses layer-shell anchors (Top, Bottom, Left, Right) and margins for resolution-independent positioning.
2. **GNOME Wayland Support**:
   - **Primary Engine**: XWayland desktop-layer window configured with `_NET_WM_WINDOW_TYPE_DESKTOP`, `_NET_WM_STATE_BELOW`, `_NET_WM_STATE_STICKY`, and `_NET_WM_DESKTOP = 0xFFFFFFFF`.
   - **Optional Native Extension (`pluvia-gnome-bridge`)**: A companion GNOME Shell extension that allows native Wayland surfaces to attach directly to GNOME's `global.window_group` behind all application windows without XWayland scaling artifacts.
3. **Pure X11**:
   - Uses native XCB connections with `_NET_WM_WINDOW_TYPE_DESKTOP` and `_NET_WM_STATE_BELOW`.

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
- Case-insensitive VFS resolver & `.ini` lexer/parser.
- FreeType in-memory font loader.
- System measures (`Time`, `CPU`, `Memory`, `Disk`, `NowPlaying`).
- Native plugin emulators (`ActionTimer`, `AudioLevel`, `Win7Audio`, `Process`, `WebParser`).
- Scoped Lua 5.1 sandbox.
- Cairo 2D renderer for all core meters.
- Tri-mode display backend (wlr-layer-shell, GNOME XWayland/extension, X11).

### Phase 2: Package Management & IPC
- Hardened Anti Zip-Slip `.rmskin` extractor.
- JSON-RPC 2.0 UNIX domain socket server with event streaming.
- `pluvia-cli` terminal client.
- Built-in default skins (Mond Clock, System Telemetry, Media Player).

### Phase 3: Pluvia Studio (Full GUI Manager)
- Desktop management application.
- Skin library tree and active instances panel.
- Visual positioning, monitor picker, and live variable editor.
- Drag-and-drop `.rmskin` installer.
