# Pluvia Universal Compatibility, Z-Order Layering & Audio-Visualizer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Achieve complete universal Rainmeter skin compatibility across arbitrary `.rmskin` suites with strict desktop window layering, fullscreen auto-pause, custom font discovery, and Cairo container masking.

**Architecture:** Extend `pluvia-core` with Rainmeter standard string formatting (`AutoScale`, `NumOfDecimals`, `Scale`), dynamic geometry section variables (`[Meter:W]`, `[Meter:H]`), `@Resources/Fonts` runtime Fontconfig loading, and Cairo container clipping. In `pluvia-daemon`, enforce strict desktop Z-order (`Layer::Background` / `_NET_WM_WINDOW_TYPE_DESKTOP`) and implement a fullscreen/gaming auto-pause watcher.

**Tech Stack:** Rust (edition 2021), Cairo, Pango/PangoCairo, Fontconfig (`fontconfig-sys` / `fontconfig`), X11 (`x11rb`), Wayland LayerShell, D-Bus (`zbus`).

**Spec:** `/home/darnell/Projects/pluvia/docs/superpowers/specs/2026-09-24-pluvia-universal-compatibility-and-features-design.md`

## Global Constraints

- Never hardcode skin-specific workarounds; all logic must adhere to the generic Rainmeter INI specification.
- Pure Rust safety: No unhandled panics on corrupted skin formulas, missing fonts, or missing parent containers.
- Zero CPU/GPU overhead when games or fullscreen windows are active.
- Transparent pixels on all desktop widgets must allow mouse clicks to pass straight through.

---

### Task 1: Universal String & Numeric Formatting Engine

**Files:**
- Modify: `crates/pluvia-core/src/ini.rs`
- Modify: `crates/pluvia-core/src/render/meter_renderer.rs`
- Create / Test: `crates/pluvia-core/tests/test_string_formatting.rs`

**Interfaces:**
- Produces: `format_meter_string(raw_val: &str, num_val: Option<f64>, meter: &MeterProps) -> String`
- Implements: `AutoScale` (1, 2, 1k, 2k), `NumOfDecimals`, `Scale`, `Percentual`, `Prefix`, `Postfix`.

- [ ] **Step 1: Write failing unit tests for String meter formatting**
  Test AutoScale byte formatting (e.g. `2967664` bytes with `AutoScale=1` -> `2.8 MB`, `AutoScale=1k` -> `2898.1 kB`), decimal truncation with `NumOfDecimals=1`, and scaling.

- [ ] **Step 2: Run test to verify it fails**
  `cargo test -p pluvia-core --test test_string_formatting`

- [ ] **Step 3: Implement string formatting helper in `meter_renderer.rs` / `ini.rs`**
  Handle numeric conversion, division by `Scale`, formatting via `AutoScale`, applying `NumOfDecimals`, prepending `Prefix`, and appending `Postfix`.

- [ ] **Step 4: Verify test passes**
  `cargo test -p pluvia-core --test test_string_formatting`

- [ ] **Step 5: Commit**
  `git commit -m "feat(core): implement universal Rainmeter string and number formatting engine"`

---

### Task 2: Dynamic Section Variables & Meter Dimension Resolution

**Files:**
- Modify: `crates/pluvia-core/src/variables.rs`
- Modify: `crates/pluvia-core/src/formulas.rs`
- Modify: `crates/pluvia-core/src/render/meter_renderer.rs`
- Create / Test: `crates/pluvia-core/tests/test_section_variables.rs`

**Interfaces:**
- Produces: `resolve_section_variables(expr: &str, meter_bounds: &HashMap<String, Rect>) -> String`
- Supports: `[MeterName:W]`, `[MeterName:H]`, `[MeterName:X]`, `[MeterName:Y]`

- [ ] **Step 1: Write failing unit tests for section variables**
  Verify `Formula=((#Bands# * [amogus:W]) - [grid:W])` evaluates correctly when meter dimensions are supplied.

- [ ] **Step 2: Run test to verify it fails**
  `cargo test -p pluvia-core --test test_section_variables`

- [ ] **Step 3: Implement section variable expansion in `variables.rs` and `formulas.rs`**
  Add regex or token matching for `\[([a-zA-Z0-9_#@]+):(W|H|X|Y|MinValue|MaxValue)\]` and substitute resolved geometric bounds before formula evaluation.

- [ ] **Step 4: Verify test passes**
  `cargo test -p pluvia-core --test test_section_variables`

- [ ] **Step 5: Commit**
  `git commit -m "feat(core): support dynamic section variable geometry in formulas"`

---

### Task 3: Skin Font Discovery & Runtime Fontconfig Registration

**Files:**
- Modify: `crates/pluvia-core/src/render/pango_text.rs`
- Modify: `crates/pluvia-core/src/vfs.rs`
- Modify: `crates/pluvia-daemon/src/runtime.rs`
- Create / Test: `crates/pluvia-core/tests/test_font_registration.rs`

**Interfaces:**
- Produces: `FontRegistry::register_skin_fonts(skin_dir: &Path)`
- Integrates with PangoCairo and Fontconfig to register `.ttf`, `.otf`, `.woff` files without requiring root or system install.

- [ ] **Step 1: Write failing tests for font registration and resolution**
  Test scanning a mock `@Resources/Fonts` directory and verifying the font family is queryable by Pango.

- [ ] **Step 2: Run test to verify it fails**
  `cargo test -p pluvia-core --test test_font_registration`

- [ ] **Step 3: Implement `register_skin_fonts` using Fontconfig / PangoFontMap**
  Add font file discovery for `@Resources/Fonts` and `#@#Fonts`, adding them to the application font config with `FcConfigAppFontAddFile`.

- [ ] **Step 4: Verify test passes**
  `cargo test -p pluvia-core --test test_font_registration`

- [ ] **Step 5: Commit**
  `git commit -m "feat(render): register skin bundled fonts dynamically with Fontconfig and Pango"`

---

### Task 4: Cairo Meter Container Masking

**Files:**
- Modify: `crates/pluvia-core/src/render/meter_renderer.rs`
- Create / Test: `crates/pluvia-core/tests/test_container_masking.rs`

**Interfaces:**
- Supports: `Container=MeterName` in any meter definition.
- Clips child meters to the alpha silhouette of the container meter.

- [ ] **Step 1: Write failing tests for container clipping**
  Create a test skin with a container meter and a child meter with `Container=Parent`, verifying rendered image mask.

- [ ] **Step 2: Run test to verify it fails**
  `cargo test -p pluvia-core --test test_container_masking`

- [ ] **Step 3: Implement container rendering and Cairo clipping mask in `meter_renderer.rs`**
  Render container meter to off-screen surface, create Cairo mask or push group with clip path, and render child meter inside.

- [ ] **Step 4: Verify test passes**
  `cargo test -p pluvia-core --test test_container_masking`

- [ ] **Step 5: Commit**
  `git commit -m "feat(render): implement Cairo container clipping masks for complex meters"`

---

### Task 5: Linux Drive Telemetry & Mount Point Mapping

**Files:**
- Modify: `crates/pluvia-core/src/measures/system.rs`
- Test: `crates/pluvia-core/tests/test_measures.rs`

**Interfaces:**
- Automatically maps `Drive=C:` -> `/`, `Drive=D:` -> `/home`, `Drive=E:` -> first external mount in `/run/media` or `/mnt`.

- [ ] **Step 1: Write unit tests for Windows drive letter mapping on Linux**
- [ ] **Step 2: Implement mount point resolver and `statvfs` queries in `system.rs`**
- [ ] **Step 3: Verify all measure tests pass cleanly**
- [ ] **Step 4: Commit**
  `git commit -m "feat(measures): map Windows drive letters to Linux mount points in FreeDiskSpace"`

---

### Task 6: Window Layering & Fullscreen Gaming Auto-Pause

**Files:**
- Modify: `crates/pluvia-daemon/src/display/layer_shell.rs`
- Modify: `crates/pluvia-daemon/src/display/x11.rs`
- Modify: `crates/pluvia-daemon/src/display/gnome_bridge.rs`
- Modify: `crates/pluvia-daemon/src/runtime.rs`

**Interfaces:**
- `FullscreenDetector`: polls or hooks `_NET_ACTIVE_WINDOW` and D-Bus inhibits.
- In `runtime.rs`, pauses timer updates when suspended and resumes when active.
- Sets `_NET_WM_WINDOW_TYPE_DESKTOP` and `_NET_WM_STATE_BELOW` on X11 / XWayland.

- [ ] **Step 1: Implement fullscreen detector and state machine in `pluvia-daemon`**
- [ ] **Step 2: Set strict desktop background z-order on Wayland layer-shell and X11 surfaces**
- [ ] **Step 3: Test pause/resume cycle and verify 0% CPU when paused**
- [ ] **Step 4: Commit**
  `git commit -m "feat(daemon): add fullscreen gaming auto-pause and enforce desktop background layer"`

---

### Task 7: End-to-End Skin Verification & Arch/CachyOS PKGBUILD

**Files:**
- Create: `packaging/PKGBUILD`
- Create: `packaging/pluvia.desktop`
- Test against: Monterey, Amogus, TronMCP

- [ ] **Step 1: Create `packaging/PKGBUILD` for Arch Linux / CachyOS**
- [ ] **Step 2: Test building package via `makepkg`**
- [ ] **Step 3: Verify Amogus, TronMCP, and Monterey render cleanly without mojibake or layout errors**
- [ ] **Step 4: Commit and push**
  `git commit -m "release: add PKGBUILD packaging and complete real-world skin compatibility fixes"`
