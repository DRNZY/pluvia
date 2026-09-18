# Task 6 Report: PangoCairo 2D Meter Renderer & Alpha Hit-Test Masks

## Summary
Implemented the 2D rendering pipeline and dynamic alpha hit-test mask engine in `pluvia-core`. Text layout and rasterization are powered by Pango and Cairo (`pangocairo`) with HarfBuzz shaping, Fontconfig dynamic application font registration (`@Resources/Fonts/*.otf|ttf`), typography styling, and case conversions. Mouse input transparency is managed via `AlphaHitMask`, generating disjoint rectangular bounding boxes for Wayland `wl_surface.set_input_region` and X11 `XFixesSetWindowShapeRegion` to enable 100% click-through on transparent desktop widget areas. The full meter rendering engine handles `Meter=String`, `Image`, `Bar`, `Roundline`, `Shape`, and `Histogram`, fully supporting measure value substitution (`%1`, `%2`), relative coordinate layouts (`10R`, `10r`), formula evaluations, aspect ratio preservation, and image tinting.

## Implemented Components

1. **PangoCairo Text Layout & Rasterization (`crates/pluvia-core/src/render/pango_text.rs`)**:
   - `PangoTextRenderer`:
     - Creates and configures `pango::Layout` directly on Cairo contexts (`pangocairo::functions::create_layout`).
     - Configures font family, font size (converted from Rainmeter point sizes to Pango units via `pango::SCALE`), font weight (`Bold`), and font style (`Italic`).
     - Computes accurate ink and logical bounding rectangles via `layout.pixel_extents()`.
     - Supports text alignments (`TextAlign::Left`, `Center`, `Right`).
     - Supports text case transformations (`TextCase::Upper`, `Lower`, `Proper`).
     - Supports text rotation angles (`Angle` in radians) and anti-aliasing control (`AntiAlias=1`).
   - `add_application_font(path: &Path) -> bool`:
     - FFI binding to Fontconfig's `FcConfigAppFontAddFile` and `FcConfigGetCurrent` (`-lfontconfig`).
     - Dynamically loads custom OTF/TTF fonts distributed in skin packages without requiring system installation.

2. **Dynamic Alpha Hit-Test Mask Engine (`crates/pluvia-core/src/render/hit_mask.rs`)**:
   - `AlphaHitMask`:
     - Extracts 1-bit solid/transparent pixel masks from Cairo surfaces (`Format::ARgb32`, `A8`, `Rgb24`).
     - Accesses pixel buffers safely via Cairo C FFI (`cairo_image_surface_get_data`).
     - `contains(x: i32, y: i32) -> bool`: accurate hit testing distinguishing interactive/opaque pixels from transparent areas.
     - `to_rectangles(&self) -> Vec<Rect>`:
       - Decomposes the 1-bit alpha mask into an optimal set of disjoint rectangles using scanline run-length encoding coalesced vertically across identical rows.
       - Formatted directly for submission to Wayland `wl_surface.set_input_region` and X11 `XFixesSetWindowShapeRegion`.

3. **Core Meter Rendering Engine (`crates/pluvia-core/src/render/meter_renderer.rs`)**:
   - `MeterRenderer` & `SkinState`:
     - Renders all visible meters in `SkinConfig.meter_order` onto an `ImageSurface` and returns the resulting `AlphaHitMask`.
     - Resolves relative coordinates (`X=10R`, `Y=0r`) and formula expressions (`#Scale# * 10`) between adjacent meters.
     - Supports solid background rectangles via `SolidColor`.
   - Meter types:
     - `Meter=String`: `%1`, `%2` measure substitutions, font face/size/color, styling, alignment, and casing.
     - `Meter=Image`: image loading (PNG, JPEG via `image` crate) with in-memory caching (`image_cache`), target dimensions `W`/`H`, aspect ratio preservation (`PreserveAspectRatio=0, 1, 2`), `ImageTint`, and `ImageAlpha`.
     - `Meter=Bar`: horizontal/vertical progress bars scaled between `MinValue` and `MaxValue`, supporting `BarColor`, `SolidColor`, and `Flip`.
     - `Meter=Roundline`: circular progress arcs and dials with `StartAngle`, `RotationAngle`, `LineLength`, `LineStart`, `LineWidth`, `LineColor`, `Solid`, and `ValueRemainder`.
     - `Meter=Shape`: vector primitive parser and renderer supporting `Rectangle` (with rounded corners), `Ellipse`, `Line`, `Curve`, `Path`, `Fill Color`, `Stroke Color`, and `StrokeWidth`.
     - `Meter=Histogram`: time-series area/line charts with rolling sample history buffers, `PrimaryColor`, and `Autoscale`.

4. **Common Render Types & Public API (`crates/pluvia-core/src/render/mod.rs` & `src/lib.rs`)**:
   - `Rect`: 2D rectangle (`x, y, width, height`) with union, containment, and coordinate conversions.
   - `Color`: RGBA normalized representation with string parsing for comma-separated RGB/RGBA (`255,255,255,128`) and hex codes (`#FFFFFF`, `#FFFFFF80`).
   - Exposed `pub mod render;` in `crates/pluvia-core/src/lib.rs`.

## Tests & Verification

- **TDD RED Phase**:
  - Authored failing unit tests in `crates/pluvia-core/tests/test_renderer.rs`.
  - Verified compilation failure (`could not find render in pluvia_core`) via `cargo test -p pluvia-core --test test_renderer`.
- **TDD GREEN Phase**:
  - Implemented `mod.rs`, `pango_text.rs`, `hit_mask.rs`, and `meter_renderer.rs`.
  - Added dependencies `cairo-rs`, `pango`, `pangocairo`, and `image` to `Cargo.toml`.
  - Verified all 11 unit and integration tests passed cleanly in `test_renderer`:
    1. `test_pango_text_surface_rendering_and_hit_mask`: Pango rendering and non-zero bounds, hit test hit/miss.
    2. `test_alpha_hit_mask_bounding_rectangles_extraction`: scanline coalescing and rectangular region extraction.
    3. `test_fontconfig_add_application_font`: dynamic font registration with Fontconfig.
    4. `test_pango_text_alignment_case_and_styling`: text alignment, casing (`Proper`, `Upper`, `Lower`), and styles.
    5. `test_meter_renderer_string_with_measure_substitution`: `%1` substitution and text rendering.
    6. `test_meter_renderer_bar_progress`: progress bar rendering and hit testing.
    7. `test_meter_renderer_image`: image loading, scaling, aspect ratio, and alpha testing.
    8. `test_meter_renderer_shape`: vector rectangle and ellipse rendering.
    9. `test_meter_renderer_roundline`: circular arc/dial rendering.
    10. `test_meter_renderer_histogram`: time-series area histogram chart rendering.
    11. `test_meter_renderer_relative_positioning`: `10R` / `0r` relative coordinate calculations.
- **Workspace-wide Verification**:
  - `cargo test --all` ran and passed all 61 tests across the entire workspace with 0 failures.
  - `cargo check --all --tests` verified 0 compiler warnings.

## Git Commits
- `d1aaa04`: `feat(core): implement PangoCairo text rendering with alpha hit-test masks`
- `7ea7a59`: `fix(render): implement Shape Path/RoundRectangle, SVG/WebP support, tinted image cache keys, and text rotation origin`

## Fix Round 1 Notes
Addressed code review findings across rendering and image pipeline:
1. **Meter=Shape Vector Primitives (`RoundRectangle` and `Path`)**:
   - Added support for `RoundRectangle` keyword alias for `Rectangle x, y, w, h, rx, ry`.
   - Added full support for `Path` primitives:
     - Rainmeter multi-part pipe syntax: `Path x, y | LineTo x, y | CurveTo x, y, cx1, cy1, [cx2, cy2] | ClosePath 1`.
     - SVG path syntax: `Path "M x y L x y C cx1 cy1 cx2 cy2 x y Z"`.
     - Evaluates exact bounding box using Cairo's `cr.path_extents()`.
2. **SVG & WebP Image Support**:
   - Added `"webp"` feature to `image` crate in `crates/pluvia-core/Cargo.toml`.
   - Added `resvg = "0.43"` dependency for native SVG rasterization into Cairo surfaces via `usvg` and `tiny_skia`.
3. **Tint-Aware Image Cache Keys**:
   - Replaced plain `PathBuf` cache key with `ImageCacheKey { path: PathBuf, tint: Option<String> }` to guarantee images rendered with different `ImageTint` color multipliers (e.g. white vs red vs green) do not corrupt or overwrite each other in cache.
4. **Text Rotation Origin & Transformed Bounds**:
   - In `PangoTextRenderer::render_text_to_context`, when `angle != 0.0`, translated Cairo coordinate system to `(render_x, render_y)` before rotating, rendered layout at local `(0.0, 0.0)`, and computed transformed axis-aligned bounding box from the 4 rotated corners.
5. **Testing**:
   - Added 4 new unit tests in `test_renderer.rs`:
     - `test_meter_renderer_shape_path_and_round_rectangle`
     - `test_meter_renderer_webp_and_svg_images`
     - `test_image_cache_distinct_tints`
     - `test_text_rotation_origin_and_bounds`
   - All 15 tests in `test_renderer` pass; 65/65 tests across workspace pass with 0 warnings.
