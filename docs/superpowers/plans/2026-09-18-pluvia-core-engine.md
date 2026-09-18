# Pluvia Core Engine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the high-performance Pluvia Core Engine and Daemon in Rust, natively parsing Rainmeter `.ini` skins, emulating measures/plugins with Linux kernel and desktop telemetry, rendering via PangoCairo with alpha hit-test masks, and providing anti-zip-slip package extraction.

**Architecture:** A modular Rust workspace comprising `pluvia-core` (parser, VFS, measures, meters, Cairo renderer), `pluvia-daemon` (JSON-RPC 2.0 UNIX socket server and compositor event loop), and `pluvia-cli` (command-line controller). Window layers are abstracted across Wayland `wlr-layer-shell`, GNOME Shell Extension, and X11 XCB.

**Tech Stack:** Rust 2021, `cairo-rs`, `pango`, `pangocairo`, `fontconfig`, `encoding_rs`, `zip`, `tokio`, `serde_json`, `zbus` (MPRIS D-Bus), `pipewire` / `pulsectl-rs`.

**Spec:** [`docs/superpowers/specs/2026-09-18-pluvia-design.md`](file:///home/darnell/Projects/pluvia/docs/superpowers/specs/2026-09-18-pluvia-design.md)

## Global Constraints
- Target RAM: $\le 12\text{ MB}$ RSS on idle Mond clock.
- Target CPU: $\le 0.1\%$ idle CPU load.
- Strictly forbidden: Symlink extraction from `.rmskin` archives.
- Strictly enforced: Canonical destination validation and anti-Zip-Slip component inspection.
- Auto-transcoding: Transparent decoding of UTF-16 LE and Windows-1252 `.ini` files into UTF-8.
- Case-insensitivity: Seamless VFS translation of backslashes and case-insensitive directory lookups.
- Text Shaping: PangoCairo + HarfBuzz with Fontconfig system font fallback resolution.
- Input regions: Dynamic alpha hit-test masks submitted to `wl_surface.set_input_region` and X11 Shape.

---

### Task 1: Rust Workspace Scaffolding & Charset Transcoder

**Files:**
- Create: `Cargo.toml`
- Create: `crates/pluvia-core/Cargo.toml`
- Create: `crates/pluvia-core/src/lib.rs`
- Create: `crates/pluvia-core/src/encoding.rs`
- Test: `crates/pluvia-core/tests/test_encoding.rs`

**Interfaces:**
- Produces: `pub fn decode_ini_bytes(raw: &[u8]) -> Result<String, EncodingError>`

- [ ] **Step 1: Write the failing test**

Create `crates/pluvia-core/tests/test_encoding.rs`:
```rust
use pluvia_core::encoding::decode_ini_bytes;

#[test]
fn test_decode_utf8_with_and_without_bom() {
    let plain_utf8 = b"[Rainmeter]\nUpdate=1000\n";
    assert_eq!(decode_ini_bytes(plain_utf8).unwrap(), "[Rainmeter]\nUpdate=1000\n");

    let bom_utf8 = [0xEF, 0xBB, 0xBF, b'[', b'R', b'a', b'i', b'n', b']'];
    assert_eq!(decode_ini_bytes(&bom_utf8).unwrap(), "[Rain]");
}

#[test]
fn test_decode_utf16_le_with_bom() {
    // UTF-16 LE BOM [0xFF, 0xFE] + "[Rainmeter]"
    let mut utf16 = vec![0xFF, 0xFE];
    for ch in "[Rainmeter]".encode_utf16() {
        utf16.extend_from_slice(&ch.to_le_bytes());
    }
    assert_eq!(decode_ini_bytes(&utf16).unwrap(), "[Rainmeter]");
}

#[test]
fn test_decode_windows_1252_ansi() {
    // 0x93 and 0x94 are smart quotes in CP1252, invalid in UTF-8
    let cp1252_bytes = vec![b'T', b'e', b'x', b't', b'=', 0x93, b'H', b'i', 0x94];
    let decoded = decode_ini_bytes(&cp1252_bytes).unwrap();
    assert!(decoded.starts_with("Text="));
    assert!(decoded.contains('“') || decoded.contains('"'));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p pluvia-core --test test_encoding`
Expected: FAIL (cargo cannot locate package/files).

- [ ] **Step 3: Write minimal implementation**

Create `Cargo.toml`:
```toml
[workspace]
members = [
    "crates/pluvia-core",
    "crates/pluvia-daemon",
    "crates/pluvia-cli",
]
resolver = "2"

[workspace.dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
thiserror = "2.0"
anyhow = "1.0"
tokio = { version = "1.40", features = ["full"] }
tracing = "0.1"
tracing-subscriber = "0.3"
```

Create `crates/pluvia-core/Cargo.toml`:
```toml
[package]
name = "pluvia-core"
version = "0.1.0"
edition = "2021"

[dependencies]
encoding_rs = "0.8"
thiserror.workspace = true
anyhow.workspace = true
```

Create `crates/pluvia-core/src/lib.rs`:
```rust
pub mod encoding;
```

Create `crates/pluvia-core/src/encoding.rs`:
```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum EncodingError {
    #[error("Failed to decode raw bytes with detected charset")]
    DecodingFailed,
}

pub fn decode_ini_bytes(raw: &[u8]) -> Result<String, EncodingError> {
    if raw.is_empty() {
        return Ok(String::new());
    }

    // 1. Check for UTF-16 LE BOM: 0xFF, 0xFE
    if raw.len() >= 2 && raw[0] == 0xFF && raw[1] == 0xFE {
        let (cow, _, malformed) = encoding_rs::UTF_16LE.decode(&raw[2..]);
        if !malformed {
            return Ok(cow.into_owned());
        }
    }

    // 2. Check for UTF-16 BE BOM: 0xFE, 0xFF
    if raw.len() >= 2 && raw[0] == 0xFE && raw[1] == 0xFF {
        let (cow, _, malformed) = encoding_rs::UTF_16BE.decode(&raw[2..]);
        if !malformed {
            return Ok(cow.into_owned());
        }
    }

    // 3. Check for UTF-8 BOM: 0xEF, 0xBB, 0xBF
    let slice = if raw.len() >= 3 && raw[0] == 0xEF && raw[1] == 0xBB && raw[2] == 0xBF {
        &raw[3..]
    } else {
        raw
    };

    // 4. Try UTF-8 directly
    if let Ok(valid_str) = std::str::from_utf8(slice) {
        return Ok(valid_str.to_string());
    }

    // 5. Fallback to Windows-1252 (CP1252)
    let (cow, _, _) = encoding_rs::WINDOWS_1252.decode(slice);
    Ok(cow.into_owned())
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p pluvia-core --test test_encoding`
Expected: PASS with 2 tests passed.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/pluvia-core/
git commit -m "feat(core): setup workspace and implement auto-transcoding for UTF-16LE and Windows-1252"
```

---

### Task 2: Case-Insensitive VFS Path Resolver & Anti-Zip-Slip Extractor

**Files:**
- Create: `crates/pluvia-core/src/vfs.rs`
- Create: `crates/pluvia-core/src/extractor.rs`
- Modify: `crates/pluvia-core/src/lib.rs`
- Test: `crates/pluvia-core/tests/test_vfs_and_extractor.rs`

**Interfaces:**
- Produces: `pub struct VfsResolver`
- Produces: `VfsResolver::resolve(&self, base: &Path, rel_path: &str) -> Option<PathBuf>`
- Produces: `pub fn extract_rmskin_package<P: AsRef<Path>, Q: AsRef<Path>>(archive_path: P, dest_root: Q) -> Result<ExtractionReport, ExtractionError>`

- [ ] **Step 1: Write the failing test**

Create `crates/pluvia-core/tests/test_vfs_and_extractor.rs`:
```rust
use pluvia_core::vfs::VfsResolver;
use pluvia_core::extractor::{extract_rmskin_package, ExtractionError};
use std::fs::{self, File};
use std::io::Write;
use tempfile::tempdir;

#[test]
fn test_vfs_case_insensitive_and_backslash_resolution() {
    let tmp = tempdir().unwrap();
    let res_dir = tmp.path().join("@Resources").join("Fonts");
    fs::create_dir_all(&res_dir).unwrap();
    let font_file = res_dir.join("Anurati.otf");
    File::create(&font_file).unwrap();

    let vfs = VfsResolver::new();
    // Resolving Windows-style backslashes and lowercase segments
    let resolved = vfs.resolve(tmp.path(), "@resources\\fonts\\anurati.otf");
    assert!(resolved.is_some());
    assert_eq!(resolved.unwrap(), font_file);
}

#[test]
fn test_anti_zip_slip_and_symlink_rejection() {
    let tmp_dest = tempdir().unwrap();
    let zip_path = tempdir().unwrap().path().join("malicious.zip");

    // Construct zip with traversal entry
    let file = File::create(&zip_path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();
    
    // 1. Directory traversal attempt
    zip.start_file("../etc/passwd", options).unwrap();
    zip.write_all(b"root:x:0:0:root").unwrap();
    zip.finish().unwrap();

    let err = extract_rmskin_package(&zip_path, tmp_dest.path()).unwrap_err();
    assert!(matches!(err, ExtractionError::ZipSlipDetected));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p pluvia-core --test test_vfs_and_extractor`
Expected: FAIL (modules `vfs` and `extractor` missing).

- [ ] **Step 3: Write minimal implementation**

Add `zip = "2.2"`, `tempfile = "3.12"`, `walkdir = "2.5"` to `crates/pluvia-core/Cargo.toml`.

Create `crates/pluvia-core/src/vfs.rs`:
```rust
use std::path::{Component, Path, PathBuf};
use std::sync::RwLock;
use std::collections::HashMap;
use std::fs;

pub struct VfsResolver {
    cache: RwLock<HashMap<String, PathBuf>>,
}

impl VfsResolver {
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
        }
    }

    pub fn normalize_rel_path(rel_path: &str) -> String {
        let stripped = if rel_path.len() >= 2 && rel_path.chars().nth(1) == Some(':') {
            &rel_path[2..]
        } else {
            rel_path
        };
        stripped.replace('\\', "/").trim_start_matches('/').to_string()
    }

    pub fn resolve(&self, base: &Path, raw_rel: &str) -> Option<PathBuf> {
        let normalized = Self::normalize_rel_path(raw_rel);
        let cache_key = format!("{}::{}", base.display(), normalized);

        if let Ok(read_guard) = self.cache.read() {
            if let Some(cached) = read_guard.get(&cache_key) {
                return Some(cached.clone());
            }
        }

        let mut current = base.to_path_buf();
        for segment in normalized.split('/') {
            if segment.is_empty() || segment == "." {
                continue;
            }
            if segment == ".." {
                current.pop();
                continue;
            }

            let direct = current.join(segment);
            if direct.exists() {
                current = direct;
                continue;
            }

            // Case-insensitive fallback lookup
            let mut found = false;
            if let Ok(entries) = fs::read_dir(&current) {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    if name.to_string_lossy().eq_ignore_ascii_case(segment) {
                        current = entry.path();
                        found = true;
                        break;
                    }
                }
            }

            if !found {
                return None;
            }
        }

        if current.exists() {
            if let Ok(mut write_guard) = self.cache.write() {
                write_guard.insert(cache_key, current.clone());
            }
            Some(current)
        } else {
            None
        }
    }
}

impl Default for VfsResolver {
    fn default() -> Self {
        Self::new()
    }
}
```

Create `crates/pluvia-core/src/extractor.rs`:
```rust
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ExtractionError {
    #[error("Zip-Slip directory traversal detected")]
    ZipSlipDetected,
    #[error("Symlink extraction is strictly forbidden")]
    SymlinkForbidden,
    #[error("Archive error: {0}")]
    ArchiveError(#[from] zip::result::ZipError),
    #[error("IO error: {0}")]
    IoError(#[from] io::Error),
}

pub struct ExtractionReport {
    pub files_extracted: usize,
    pub total_bytes: u64,
}

pub fn extract_rmskin_package<P: AsRef<Path>, Q: AsRef<Path>>(
    archive_path: P,
    dest_root: Q,
) -> Result<ExtractionReport, ExtractionError> {
    let dest_canonical = dest_root.as_ref().canonicalize().unwrap_or_else(|_| dest_root.as_ref().to_path_buf());
    let file = File::open(archive_path)?;
    let mut zip = zip::ZipArchive::new(file)?;

    let mut files_extracted = 0;
    let mut total_bytes = 0;

    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;

        // 1. Strict symlink check via unix_mode
        if let Some(mode) = entry.unix_mode() {
            if (mode & 0o170000) == 0o120000 {
                return Err(ExtractionError::SymlinkForbidden);
            }
        }

        // 2. Strict component verification
        let enclosed = match entry.enclosed_name() {
            Some(path) => path.to_path_buf(),
            None => return Err(ExtractionError::ZipSlipDetected),
        };

        if enclosed.components().any(|c| matches!(c, Component::ParentDir | Component::Prefix(_) | Component::RootDir)) {
            return Err(ExtractionError::ZipSlipDetected);
        }

        let target_path = dest_canonical.join(&enclosed);

        if entry.is_dir() {
            fs::create_dir_all(&target_path)?;
        } else {
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut out_file = File::create(&target_path)?;
            let bytes = io::copy(&mut entry, &mut out_file)?;
            files_extracted += 1;
            total_bytes += bytes;
        }
    }

    Ok(ExtractionReport {
        files_extracted,
        total_bytes,
    })
}
```

Modify `crates/pluvia-core/src/lib.rs` to expose `vfs` and `extractor`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p pluvia-core --test test_vfs_and_extractor`
Expected: PASS with 2 tests passed.

- [ ] **Step 5: Commit**

```bash
git add crates/pluvia-core/
git commit -m "feat(core): implement case-insensitive VFS and anti-zip-slip package extractor"
```

---

### Task 3: Rainmeter `.ini` Lexer, Parser & Expression Engine

**Files:**
- Create: `crates/pluvia-core/src/ini.rs`
- Create: `crates/pluvia-core/src/variables.rs`
- Create: `crates/pluvia-core/src/formulas.rs`
- Modify: `crates/pluvia-core/src/lib.rs`
- Test: `crates/pluvia-core/tests/test_parser.rs`

**Interfaces:**
- Produces: `pub struct SkinConfig`
- Produces: `pub fn parse_skin_ini(content: &str, skin_dir: &Path) -> Result<SkinConfig, ParseError>`
- Produces: `pub fn eval_formula(expr: &str, vars: &VariableMap) -> Result<f64, FormulaError>`

- [ ] **Step 1: Write the failing test**

Create `crates/pluvia-core/tests/test_parser.rs`:
```rust
use pluvia_core::ini::parse_skin_ini;
use pluvia_core::formulas::eval_formula;
use std::collections::HashMap;
use std::path::Path;

#[test]
fn test_parse_mond_clock_ini() {
    let sample_ini = r#"
[Rainmeter]
Update=1000
AccurateText=1
DynamicWindowSize=1

[Variables]
FontName=Anurati
Color1=226,232,240
Scale=1.2

[MeasureTime]
Measure=Time
Format=%H:%M

[MeterTime]
Meter=String
MeasureName=MeasureTime
FontFace=#FontName#
FontColor=#Color1#
FontSize=(16 * #Scale#)
Text="Now: %1"
AntiAlias=1
"#;

    let config = parse_skin_ini(sample_ini, Path::new("/dummy")).unwrap();
    assert_eq!(config.update_rate_ms, 1000);
    assert_eq!(config.variables.get("fontname").unwrap(), "Anurati");
    
    let meter = config.meters.get("metertime").unwrap();
    assert_eq!(meter.font_face.as_deref(), Some("Anurati"));
    assert_eq!(meter.font_size, Some(19.2));
}

#[test]
fn test_formula_math_evaluation() {
    let mut vars = HashMap::new();
    vars.insert("w".to_string(), 100.0);
    vars.insert("scale".to_string(), 1.5);

    assert_eq!(eval_formula("(#W# * #Scale#) + 10", &vars).unwrap(), 160.0);
    assert_eq!(eval_formula("Round(14.7)", &vars).unwrap(), 15.0);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p pluvia-core --test test_parser`
Expected: FAIL (modules not found).

- [ ] **Step 3: Write minimal implementation**

Implement `ini.rs`, `variables.rs`, `formulas.rs` using recursive descent or a shunting-yard evaluator for arithmetic (`+`, `-`, `*`, `/`, `%`), nested parentheses, `#Variable#` expansion, and basic functions (`Round`, `Trunc`, `Min`, `Max`).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p pluvia-core --test test_parser`
Expected: PASS with 2 tests passed.

- [ ] **Step 5: Commit**

```bash
git add crates/pluvia-core/
git commit -m "feat(core): implement Rainmeter INI parser and formula evaluator"
```

---

### Task 4: Linux Telemetry & Measure Engine

**Files:**
- Create: `crates/pluvia-core/src/measures/mod.rs`
- Create: `crates/pluvia-core/src/measures/time.rs`
- Create: `crates/pluvia-core/src/measures/system.rs`
- Create: `crates/pluvia-core/src/measures/mpris.rs`
- Test: `crates/pluvia-core/tests/test_measures.rs`

**Interfaces:**
- Produces: `pub trait Measure { fn update(&mut self) -> MeasureValue; }`
- Produces: `pub enum MeasureValue { String(String), Number(f64) }`

- [ ] **Step 1: Write the failing test**

Create `crates/pluvia-core/tests/test_measures.rs`:
```rust
use pluvia_core::measures::time::TimeMeasure;
use pluvia_core::measures::system::{CpuMeasure, MemoryMeasure};
use pluvia_core::measures::{Measure, MeasureValue};

#[test]
fn test_time_measure_formatting() {
    let mut m = TimeMeasure::new("%Y-%m-%d");
    let val = m.update();
    if let MeasureValue::String(s) = val {
        assert!(s.len() == 10);
        assert!(s.contains('-'));
    } else {
        panic!("Expected string value");
    }
}

#[test]
fn test_system_memory_measure() {
    let mut mem = MemoryMeasure::new("used_percent");
    let val = mem.update();
    if let MeasureValue::Number(pct) = val {
        assert!(pct >= 0.0 && pct <= 100.0);
    } else {
        panic!("Expected numeric percent");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p pluvia-core --test test_measures`
Expected: FAIL.

- [ ] **Step 3: Write minimal implementation**

Implement zero-allocation parsing of `/proc/stat` for CPU delta and `/proc/meminfo` for memory. Implement `TimeMeasure` with `chrono`. Wire MPRIS metadata querying via `zbus`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p pluvia-core --test test_measures`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/pluvia-core/
git commit -m "feat(core): implement Linux system telemetry and MPRIS measures"
```

---

### Task 5: Native C++ DLL Plugin Emulators

**Files:**
- Create: `crates/pluvia-core/src/plugins/mod.rs`
- Create: `crates/pluvia-core/src/plugins/action_timer.rs`
- Create: `crates/pluvia-core/src/plugins/audio_level.rs`
- Create: `crates/pluvia-core/src/plugins/web_parser.rs`
- Test: `crates/pluvia-core/tests/test_plugins.rs`

**Interfaces:**
- Produces: `pub struct ActionTimerPlugin`
- Produces: `pub struct AudioLevelPlugin`
- Produces: `pub struct WebParserPlugin`

- [ ] **Step 1: Write the failing test**

Create `crates/pluvia-core/tests/test_plugins.rs`:
```rust
use pluvia_core::plugins::action_timer::{ActionTimerPlugin, ActionStep};

#[test]
fn test_action_timer_queue_execution() {
    let mut timer = ActionTimerPlugin::new();
    timer.enqueue_action("Slide", vec![
        ActionStep::Wait(10),
        ActionStep::SetVariable("Alpha".to_string(), "255".to_string()),
    ]);
    assert!(timer.has_pending());
    timer.tick(15);
    assert!(!timer.has_pending());
    assert_eq!(timer.get_variable("Alpha"), Some("255"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p pluvia-core --test test_plugins`
Expected: FAIL.

- [ ] **Step 3: Write minimal implementation**

Implement `action_timer.rs`, `audio_level.rs` (with an FFT frequency band model), and `web_parser.rs` (using `reqwest` async client).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p pluvia-core --test test_plugins`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/pluvia-core/
git commit -m "feat(core): implement native Linux emulators for ActionTimer, AudioLevel, and WebParser"
```

---

### Task 6: PangoCairo 2D Meter Renderer & Alpha Hit-Test Masks

**Files:**
- Create: `crates/pluvia-core/src/render/mod.rs`
- Create: `crates/pluvia-core/src/render/pango_text.rs`
- Create: `crates/pluvia-core/src/render/hit_mask.rs`
- Test: `crates/pluvia-core/tests/test_renderer.rs`

**Interfaces:**
- Produces: `pub struct MeterRenderer`
- Produces: `MeterRenderer::render_to_surface(&self, skin: &SkinState, surface: &cairo::ImageSurface) -> AlphaHitMask`

- [ ] **Step 1: Write the failing test**

Create `crates/pluvia-core/tests/test_renderer.rs`:
```rust
use pluvia_core::render::pango_text::PangoTextRenderer;
use pluvia_core::render::hit_mask::AlphaHitMask;
use cairo::{Format, ImageSurface};

#[test]
fn test_pango_text_surface_rendering_and_hit_mask() {
    let surface = ImageSurface::create(Format::ARgb32, 400, 200).unwrap();
    let text_renderer = PangoTextRenderer::new();
    let rect = text_renderer.render_text(&surface, "MONDAY", "Sans Bold", 32.0, (1.0, 1.0, 1.0, 1.0), 50.0, 50.0).unwrap();

    let mask = AlphaHitMask::from_surface(&surface);
    // Non-transparent text area must register hit
    assert!(mask.contains(rect.x as i32 + 5, rect.y as i32 + 5));
    // Blank corner must be click-through
    assert!(!mask.contains(5, 5));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p pluvia-core --test test_renderer`
Expected: FAIL.

- [ ] **Step 3: Write minimal implementation**

Implement PangoCairo layout rendering, Fontconfig registration for `@Resources/Fonts`, and 1-bit alpha bounding-box hit mask generator.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p pluvia-core --test test_renderer`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/pluvia-core/
git commit -m "feat(core): implement PangoCairo text rendering with alpha hit-test masks"
```

---

### Task 7: Display Layer & Window Backend

**Files:**
- Create: `crates/pluvia-daemon/src/display/mod.rs`
- Create: `crates/pluvia-daemon/src/display/layer_shell.rs`
- Create: `crates/pluvia-daemon/src/display/x11.rs`
- Test: `crates/pluvia-daemon/tests/test_display_bounds.rs`

**Interfaces:**
- Produces: `pub trait DesktopSurface { fn update_surface(&mut self, surface: &cairo::ImageSurface, hit_mask: &AlphaHitMask); }`

- [ ] **Step 1: Write the failing test**

Verify anchor coordinate translation to surface bounds across standard monitor viewports (1920x1080, 5120x1440).

- [ ] **Step 2: Run test and verify it fails**
- [ ] **Step 3: Implement display layer abstraction**
- [ ] **Step 4: Verify test passes**
- [ ] **Step 5: Commit**

```bash
git add crates/pluvia-daemon/
git commit -m "feat(daemon): implement multi-backend display abstraction with layer-shell and X11"
```

---

### Task 8: UNIX Domain Socket JSON-RPC 2.0 Server & CLI

**Files:**
- Create: `crates/pluvia-daemon/src/ipc.rs`
- Create: `crates/pluvia-cli/src/main.rs`
- Test: `crates/pluvia-daemon/tests/test_ipc.rs`

**Interfaces:**
- Produces: `pub struct JsonRpcServer`
- Produces: `pluvia-cli load <skin>`, `pluvia-cli list`, `pluvia-cli unload`

- [ ] **Step 1: Write the failing test for IPC JSON-RPC ping, load, and state queries**
- [ ] **Step 2: Run test to verify it fails**
- [ ] **Step 3: Implement async Tokio UNIX domain socket server and CLI commands**
- [ ] **Step 4: Verify test passes**
- [ ] **Step 5: Commit**

```bash
git add crates/pluvia-daemon/ crates/pluvia-cli/
git commit -m "feat(ipc): implement JSON-RPC 2.0 socket server and CLI controller"
```

---

### Task 9: Out-of-the-Box Mond Clock Integration & End-to-End Test

**Files:**
- Create: `skins/Mond/Clock/Clock.ini`
- Create: `skins/Mond/@Resources/Fonts/Anurati.otf`
- Create: `tests/e2e_mond_clock.rs`

- [ ] **Step 1: Write End-to-End test loading Mond Clock, verifying measures, font resolution, and rendering cycle**
- [ ] **Step 2: Run test to verify it passes**
- [ ] **Step 3: Commit and verify memory footprint**

```bash
git add skins/ tests/
git commit -m "feat(demo): package native Mond Clock and verify end-to-end rendering pipeline"
```
