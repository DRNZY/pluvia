# Task 6 Brief: PangoCairo 2D Meter Renderer & Alpha Hit-Test Masks

## Overview
Implement the 2D rendering pipeline and dynamic alpha hit-test mask engine in `pluvia-core`:
1. `crates/pluvia-core/src/render/pango_text.rs`:
   - Text layout and rasterization using Pango and Cairo (`pangocairo`).
   - Resolves font families via Fontconfig, handles system font fallbacks, OpenType ligatures, text alignments (`Left`, `Center`, `Right`), text styles (`Bold`, `Italic`), and string case conversions (`Upper`, `Lower`, `Proper`).
   - Dynamic application font registration (`add_application_font(path: &Path)`) for `@Resources/Fonts/*.otf|ttf`.
2. `crates/pluvia-core/src/render/hit_mask.rs`:
   - `AlphaHitMask` generating dynamic mouse hit-test regions from transparent Cairo surfaces or meter bounding boxes.
   - Accurately distinguishes interactive/opaque pixels from transparent empty areas (`contains(x, y) -> bool`).
   - Provides `to_rectangles(&self) -> Vec<Rect>` suitable for submitting to `wl_surface.set_input_region` (Wayland) and `XFixesSetWindowShapeRegion` (X11) so transparent areas are 100% click-through.
3. `crates/pluvia-core/src/render/meter_renderer.rs`:
   - Full meter rendering engine for:
     - `Meter=String`: text with measure value `%1`, `%2` substitutions, font styling, and bounding calculation.
     - `Meter=Image`: image loading (PNG, JPEG, WebP, SVG) with width, height, aspect ratio preservation (`PreserveAspectRatio=1`), `ImageTint`, and `ImageAlpha`.
     - `Meter=Bar`: horizontal/vertical progress bars scaled between MinValue/MaxValue.
     - `Meter=Roundline`: circular progress arcs and dials with `StartAngle`, `RotationAngle`, and `LineLength`.
     - `Meter=Shape`: vector primitives (`Rectangle`, `RoundRectangle`, `Ellipse`, `Path`) with fills and strokes.
     - `Meter=Histogram`: time-series area/line charts.
4. Export `pub mod render;` in `crates/pluvia-core/src/lib.rs`.

## Target Files
- `crates/pluvia-core/Cargo.toml` (add `cairo-rs = "0.20"`, `pango = "0.20"`, `pangocairo = "0.20"`, `image = "0.25"`)
- `crates/pluvia-core/src/render/mod.rs`
- `crates/pluvia-core/src/render/pango_text.rs`
- `crates/pluvia-core/src/render/hit_mask.rs`
- `crates/pluvia-core/src/render/meter_renderer.rs`
- `crates/pluvia-core/src/lib.rs`
- `crates/pluvia-core/tests/test_renderer.rs`

## Instructions
1. Follow TDD: create failing unit tests in `crates/pluvia-core/tests/test_renderer.rs` covering:
   - Pango text layout rendering to a Cairo `ImageSurface` (verifying non-zero ink rectangle and correct dimensions).
   - `AlphaHitMask` hit testing: verify text/rendered pixels return `contains == true` while transparent background returns `false`.
   - Bounding rectangle extraction from `AlphaHitMask` for Wayland/X11 input region submission.
   - Meter rendering for `String`, `Bar`, `Image`, and `Shape`.
2. Verify failure with `cargo test -p pluvia-core --test test_renderer`.
3. Implement `pango_text.rs`, `hit_mask.rs`, `meter_renderer.rs`, and `mod.rs`.
4. Verify all tests pass with `cargo test -p pluvia-core --test test_renderer`.
5. Commit with message: `feat(core): implement PangoCairo text rendering with alpha hit-test masks`
6. Write completion report to `.superpowers/sdd/2026-09-18-pluvia-core-engine/task-6-report.md`.
7. Return status under 15 lines.
