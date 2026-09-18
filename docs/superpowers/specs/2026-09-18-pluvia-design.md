# Pluvia: Rainmeter-Compatible Desktop Widget Engine for Linux
**Design Specification**  
*Date: 2026-09-18*  
*Status: Draft / Under Review*

---

## 1. Executive Summary

Pluvia is an open-source, ultra-lightweight, native Linux desktop widget engine and ecosystem engineered to natively execute the vast catalog of existing Windows Rainmeter skins (`.rmskin` and `.ini`). 

Pluvia is architected across two decoupled tiers:
1. **Pluvia Lite (`pluvia-daemon` & `pluvia-cli`)**: A high-performance, single-binary daemon written in Rust that operates under **12 MB RAM** with $\le 0.1\%$ idle CPU. It directly ingests Rainmeter `.ini` syntax, evaluates measures against Linux system interfaces (`/proc`, D-Bus), dynamically binds fonts into memory, and draws transparent desktop overlay windows via Cairo and native Wayland/X11 backends.
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
   │  │   Skin Parser (.ini) │  │   Telemetry Engine  │  │ Sandbox Zip    │  │
   │  │  • Section Lexer     │  │  • /proc/stat (CPU) │  │  Extractor     │  │
   │  │  • Variable Expander │  │  • /proc/meminfo    │  │  • Zip-Slip    │  │
   │  │  • Calc Evaluator    │  │  • MPRIS D-Bus      │  │    Prevention  │  │
   │  └──────────┬───────────┘  └──────────┬──────────┘  └────────────────┘  │
   │             │                         │                                 │
   │             ▼                         ▼                                 │
   │  ┌───────────────────────────────────────────────┐  ┌────────────────┐  │
   │  │                Measure Engine                 │  │ Sandboxed Lua  │  │
   │  │  Time | CPU | Memory | Disk | Net | NowPlaying│◄─┤ 5.1 Runtime    │  │
   │  └──────────────────────┬────────────────────────┘  └────────────────┘  │
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
   │  │          Display Layer Abstraction            │                      │
   │  │  • Wayland: wlr-layer-shell (anchors/margins) │                      │
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

## 4. Rainmeter Compatibility Specification

### 4.1 Parser & Variables
- **File Format**: Standard Windows `.ini` syntax (case-insensitive keys and section names, semicolon comments).
- **Special Folders**: Automatic resolution of `#@#` to the skin suite's `@Resources/` directory, `#SKINSPATH#`, `#CURRENTPATH#`.
- **Dynamic Variables**: Sections flagged with `DynamicVariables=1` re-evaluate `#Variables#` on every tick.
- **Formulas**: Math expression evaluator supporting arithmetic (`+`, `-`, `*`, `/`, `%`, `**`), logic (`=`, `<>`, `<`, `>`, `<=`, `>=`), and standard functions (`Round`, `Trunc`, `Sin`, `Cos`, `Min`, `Max`).

### 4.2 Measures (Data Inputs)
- **`Measure=Time`**: Formats local date and time using Windows/C `strftime` format strings (`%A`, `%d`, `%B`, `%Y`, `%H`, `%M`, `%S`).
- **`Measure=CPU`**: Direct zero-allocation delta parsing of `/proc/stat` across all cores or specific core indexes.
- **`Measure=Memory`**: Reads `/proc/meminfo` to calculate `Total`, `Used`, `Free`, and percentage.
- **`Measure=FreeDiskSpace` / `Drive`**: Invokes `statvfs` on target mount points (`/`, `/home`, etc.).
- **`Measure=NetIn` / `NetOut`**: Delta bandwidth tracking via `/proc/net/dev`.
- **`Measure=NowPlaying` / `WebNowPlaying`**: Queries D-Bus `org.mpris.MediaPlayer2.*` for real-time track metadata (Artist, Title, Album, AlbumArt URI, PlaybackStatus, Position, Duration).
- **`Measure=Calc`**: Formula evaluation based on values of prerequisite measures.
- **`Measure=Uptime`**: Reads `/proc/uptime`.
- **`Measure=Script`**: Executes Rainmeter Lua 5.1 scripts via an embedded sandboxed Lua runtime (`Initialize()`, `Update()`).

### 4.3 Meters (Visual Primitives)
- **`Meter=String`**:
  - Rendered via Cairo and FreeType.
  - Sub-pixel antialiasing, custom font matching from `@Resources/Fonts/`, text case transforms (`Upper`, `Lower`, `Proper`), font styles (Bold, Italic), text alignment (`Left`, `Center`, `Right`).
- **`Meter=Image`**:
  - Cairo image surface loading PNG, JPEG, SVG, WebP.
  - Aspect ratio preservation, explicit width/height matrix scaling, `ImageTint` color multiplication, `ImageAlpha`.
- **`Meter=Bar`**:
  - Horizontal and vertical progress bars bound to a measure's scaled range (`MinValue` to `MaxValue`).
- **`Meter=Roundline`**:
  - Circular and radial progress arcs, clock hands, and analog dials with start angle, rotation range, line length, and fill.
- **`Meter=Histogram`**:
  - Rolling time-series area charts and line graphs plotting historical measure values.
- **`Meter=Shape`**:
  - Vector primitives: `Rectangle`, `RoundRectangle`, `Ellipse`, `Path`, and linear/radial gradient fills.

### 4.4 Rainmeter Bangs & Actions
- Built-in support for mouse events: `LeftMouseUpAction`, `LeftMouseDownAction`, `RightMouseUpAction`, `MouseOverAction`, `MouseLeaveAction`.
- Bang interpreter supporting:
  - `!SetVariable <Var> <Value>`
  - `!UpdateMeter <Meter>`
  - `!Redraw`
  - `!ToggleConfig <Config> <File>`
  - `!Show` / `!Hide`
  - `!Refresh`
  - Shell execution of desktop commands.

---

## 5. Architectural Improvements

### 5.1 Wayland Layer-Shell & X11 Desktop Window Abstraction
Rather than fragile global screen coordinates, Pluvia abstracts window positioning through a unified compositor layer:
- **Wayland**: Employs `wlr-layer-shell` targeting `ZWLR_LAYER_SHELL_V1_LAYER_BOTTOM` or `BACKGROUND`.
  - Position is defined as **Anchor edges** (Top, Bottom, Left, Right) with pixel margins.
  - Supports fractional scaling ($125\%, 150\%$) and dynamic multi-monitor hotplugging without displacement.
  - Input pass-through (`set_input_region(None)`) enabled when "Click Through" is active.
- **X11**: Sets `_NET_WM_WINDOW_TYPE_DESKTOP` with `_NET_WM_STATE_BELOW`, `_NET_WM_STATE_STICKY`, and `_NET_WM_DESKTOP = 0xFFFFFFFF` (all workspaces).

### 5.2 JSON-RPC 2.0 IPC Protocol with Pub/Sub
Communication over `/run/user/<uid>/pluvia.sock` follows strict JSON-RPC 2.0:
- **Methods**:
  - `pluvia.loadSkin { path: "Mond/Clock/Clock.ini", anchor: "TopCenter", margin_y: 100 }`
  - `pluvia.unloadSkin { id: "Mond/Clock" }`
  - `pluvia.listSkins {}`
  - `pluvia.getActiveInstances {}`
  - `pluvia.setVariable { skin_id: "Mond/Clock", key: "Color1", value: "255,255,255" }`
  - `pluvia.importPackage { file_path: "/path/to/skin.rmskin" }`
- **Pub/Sub Notifications**:
  - `notify.skinLoaded`, `notify.skinUnloaded`, `notify.meterUpdated`, `notify.packageExtractProgress`.

### 5.3 Sandboxed Package Extraction (Anti Zip-Slip)
- `.rmskin` files are unpacked in an asynchronous worker thread.
- **Path Sanitization**: Every file entry is strictly verified using canonical path validation:
  ```rust
  let out_path = target_dir.join(entry_path);
  if !out_path.starts_with(&target_dir) {
      return Err(SecurityError::ZipSlipDetected);
  }
  ```
- Strips any Windows-specific invalid path separators, normalizes case, and emits continuous progress events over IPC.

### 5.4 Sandboxed Lua 5.1 / LuaJIT Runtime
- Embedded Lua 5.1 runtime.
- Dangerous standard library functions (`os.execute`, `os.remove`, `io.popen`, `loadfile`) are replaced with restricted sandboxed shims.
- Provides native Rainmeter `SKIN` global table (`SKIN:GetMeasure()`, `SKIN:GetVariable()`, `SKIN:Bang()`).

---

## 6. Implementation Scope & Milestones

### Phase 1: Core Engine & Parser (`pluvia-daemon`)
- Rust project structure (`pluvia-core`, `pluvia-daemon`, `pluvia-cli`).
- Lexer and parser for `.ini` files, variables, and math formulas.
- FreeType in-memory font loader.
- System measures (`Time`, `CPU`, `Memory`, `Disk`, `NowPlaying`).
- Cairo 2D renderer for `String`, `Image`, `Bar`, `Roundline`, `Shape`.
- X11 and Wayland transparent window backends.

### Phase 2: Package Management & IPC
- Zip-Slip safe `.rmskin` extractor with async progress.
- UNIX domain socket server with JSON-RPC 2.0 and event streaming.
- `pluvia-cli` tool commands.
- Bundled default skins: Mond Clock, System Telemetry, Media Player.

### Phase 3: Pluvia Studio (Full GUI Manager)
- Desktop management application.
- Skin library tree and active instances panel.
- Visual positioning, monitor picker, and live variable editor.
- Drag-and-drop `.rmskin` installer.

---

## 7. Testing & Verification

1. **Parser & Expression Unit Tests**:
   - Verify parsing of real-world skins (Mond, Elegance, Simplistic Clock).
   - Test variable inheritance, `#@#` macro resolution, and math formula precedence.
2. **Security & Sandbox Tests**:
   - Malformed `.rmskin` with `../../` path traversal rejected immediately.
   - Lua scripts attempting `os.execute` blocked safely.
3. **Telemetry Benchmarks**:
   - Verify CPU and RAM readings match `htop` and `/proc/meminfo`.
   - MPRIS D-Bus track change events received under 5ms.
4. **Footprint & Performance Gate**:
   - `pluvia-daemon` RSS memory verified $\le 12\text{ MB}$ when running Mond clock.
   - Idle CPU verified $\le 0.1\%$.
