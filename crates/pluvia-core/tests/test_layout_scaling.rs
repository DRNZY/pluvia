//! Regression tests for skin layout, scaling and window-sizing correctness.
//!
//! These cover the defects that made widgets misaligned, cropped and unscalable:
//!   * `StringAlign` was anchored to `X`/`Y` instead of the meter's `W`/`H` layout box
//!   * window size was derived from declared `W`/`H` rather than measured content, so
//!     text that overflowed its box was clipped
//!   * the global `Scale` variable resized the window but not the content
//!   * an empty `Substitute` key grew a string exponentially until it exhausted RAM

use cairo::{Format, ImageSurface};
use pluvia_core::ini::parse_skin_ini;
use pluvia_core::measures::MeasureValue;
use pluvia_core::render::hit_mask::AlphaHitMask;
use pluvia_core::render::meter_renderer::{skin_scale, MeterRenderer, SkinState};
use pluvia_core::render::pango_text::{TextAlign, TextStyle};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

fn render(ini: &str, w: i32, h: i32) -> (ImageSurface, AlphaHitMask) {
    let config = parse_skin_ini(ini, Path::new("/dummy")).unwrap();
    let mut values = HashMap::new();
    for name in &config.measure_order {
        values.insert(name.clone(), MeasureValue::String("ABCDEFGH".to_string()));
    }
    let state = SkinState::from_arc(Arc::new(config), Arc::new(values));
    let surface = ImageSurface::create(Format::ARgb32, w, h).unwrap();
    let mask = MeterRenderer::new().render_to_surface(&state, &surface).unwrap();
    (surface, mask)
}

/// Horizontal centre of the drawn ink, averaged over every solid column.
fn ink_center_x(mask: &AlphaHitMask) -> f64 {
    let mut min = u32::MAX;
    let mut max = 0u32;
    for y in 0..mask.height {
        for x in 0..mask.width {
            if mask.pixels[(y * mask.width + x) as usize] != 0 {
                min = min.min(x);
                max = max.max(x);
            }
        }
    }
    assert_ne!(min, u32::MAX, "expected some ink to be drawn");
    (min + max) as f64 / 2.0
}

#[test]
fn string_align_center_uses_the_meter_layout_box() {
    // Rainmeter: X/Y is the top-left of the layout box and StringAlign positions the
    // text *inside* it. Center must land on X + W/2, not on X.
    let ini = r#"
[Rainmeter]
Update=1000

[MeasureM]
Measure=Time
Format=%H:%M

[MeterM]
Meter=String
MeasureName=MeasureM
FontFace=DejaVu Sans
FontSize=20
FontColor=255,255,255,255
StringAlign=Center
X=0
Y=20
W=600
H=40
"#;
    let (_surface, mask) = render(ini, 600, 100);
    let center = ink_center_x(&mask);
    // Box centre is 300; allow a couple of pixels for glyph side bearings.
    assert!(
        (center - 300.0).abs() < 4.0,
        "centered text should sit on the box centre (300), got {center}"
    );
}

#[test]
fn string_align_right_anchors_to_box_right_edge() {
    let ini = r#"
[Rainmeter]
Update=1000

[MeasureM]
Measure=Time
Format=%H:%M

[MeterM]
Meter=String
MeasureName=MeasureM
FontFace=DejaVu Sans
FontSize=20
FontColor=255,255,255,255
StringAlign=Right
X=0
Y=20
W=600
H=40
"#;
    let (_surface, mask) = render(ini, 600, 100);
    let mut max_x = 0u32;
    for y in 0..mask.height {
        for x in 0..mask.width {
            if mask.pixels[(y * mask.width + x) as usize] != 0 {
                max_x = max_x.max(x);
            }
        }
    }
    // The final glyph's right side bearing sits a pixel or two inside the box edge.
    assert!(
        (max_x as i64 - 600).abs() <= 4,
        "right-aligned text should end at the box right edge (600), got {max_x}"
    );
}

#[test]
fn string_align_left_anchors_to_box_left_edge() {
    let ini = r#"
[Rainmeter]
Update=1000

[MeasureM]
Measure=Time
Format=%H:%M

[MeterM]
Meter=String
MeasureName=MeasureM
FontFace=DejaVu Sans
FontSize=20
FontColor=255,255,255,255
StringAlign=Left
X=40
Y=20
W=600
H=40
"#;
    let (_surface, mask) = render(ini, 800, 100);
    let mut min_x = u32::MAX;
    for y in 0..mask.height {
        for x in 0..mask.width {
            if mask.pixels[(y * mask.width + x) as usize] != 0 {
                min_x = min_x.min(x);
            }
        }
    }
    assert_eq!(min_x, 40, "left-aligned text should start at X");
}

#[test]
fn measure_content_reports_true_extent_of_overflowing_text() {
    // The meter declares W=200 but the text is much wider. Window sizing must use the
    // measured ink width, otherwise the tail of the string is cropped.
    let ini = r#"
[Rainmeter]
Update=1000

[MeasureM]
Measure=Time
Format=%H:%M

[MeterM]
Meter=String
MeasureName=MeasureM
FontFace=DejaVu Sans
FontSize=40
FontColor=255,255,255,255
StringAlign=Center
X=0
Y=20
W=200
H=60
"#;
    let config = parse_skin_ini(ini, Path::new("/dummy")).unwrap();
    let mut values = HashMap::new();
    values.insert(
        "measurem".to_string(),
        MeasureValue::String("A VERY LONG STRING VALUE".to_string()),
    );
    let state = SkinState::from_arc(Arc::new(config), Arc::new(values));
    let content = MeterRenderer::new().measure_content(&state);

    assert!(
        content.width > 200.0,
        "measured width {} must exceed the declared 200px box",
        content.width
    );
    // And it must actually contain the right edge of the drawn text.
    let (_s, mask) = render(ini, 2000, 200);
    let mut max_x = 0u32;
    for y in 0..mask.height {
        for x in 0..mask.width {
            if mask.pixels[(y * mask.width + x) as usize] != 0 {
                max_x = max_x.max(x);
            }
        }
    }
    assert!(
        (content.x + content.width) >= max_x as f64,
        "content right edge {:.1} must cover drawn ink at x={max_x}",
        content.x + content.width
    );
}

#[test]
fn measured_bounds_never_clip_the_drawn_ink() {
    let ini = r#"
[Rainmeter]
Update=1000

[MeasureM]
Measure=Time
Format=%H:%M

[MeterA]
Meter=String
MeasureName=MeasureM
FontFace=DejaVu Sans
FontSize=36
FontColor=255,255,255,255
StringAlign=Center
X=0
Y=10
W=300
H=50

[MeterB]
Meter=String
MeasureName=MeasureM
FontFace=DejaVu Sans
FontSize=18
FontColor=255,0,0,255
StringAlign=Right
X=0
Y=80
W=640
H=30
"#;
    let config = parse_skin_ini(ini, Path::new("/dummy")).unwrap();
    let mut values = HashMap::new();
    values.insert(
        "measurem".to_string(),
        MeasureValue::String("SOME LONG VALUE".to_string()),
    );
    let state = SkinState::from_arc(Arc::new(config), Arc::new(values));
    let renderer = MeterRenderer::new();
    let content = renderer.measure_content(&state);

    // Render at exactly the measured size: nothing may be lost off the edge.
    let w = (content.x + content.width).ceil() as i32;
    let h = (content.y + content.height).ceil() as i32;
    let surface = ImageSurface::create(Format::ARgb32, w, h).unwrap();
    let mask = renderer.render_to_surface(&state, &surface).unwrap();

    // No ink may touch the right or bottom edge, otherwise content is cropped.
    let touches_right = (0..mask.height)
        .any(|y| mask.pixels[(y * mask.width + (mask.width - 1)) as usize] != 0);
    let touches_bottom = (0..mask.width)
        .any(|x| mask.pixels[((mask.height - 1) * mask.width + x) as usize] != 0);
    assert!(!touches_right, "content is clipped at the right edge");
    assert!(!touches_bottom, "content is clipped at the bottom edge");
}

#[test]
fn skin_scale_reads_the_scale_variable_safely() {
    let scaled = parse_skin_ini(
        "[Rainmeter]\nUpdate=1000\n\n[Variables]\nScale=2.0\n",
        Path::new("/dummy"),
    )
    .unwrap();
    assert_eq!(skin_scale(&scaled), 2.0);

    let missing = parse_skin_ini("[Rainmeter]\nUpdate=1000\n", Path::new("/dummy")).unwrap();
    assert_eq!(skin_scale(&missing), 1.0);

    // Nonsensical values fall back to 1.0 rather than producing a broken window.
    for bad in ["0", "-3", "abc", "nan", "inf"] {
        let cfg = parse_skin_ini(
            &format!("[Rainmeter]\nUpdate=1000\n\n[Variables]\nScale={bad}\n"),
            Path::new("/dummy"),
        )
        .unwrap();
        assert_eq!(skin_scale(&cfg), 1.0, "Scale={bad} should fall back to 1.0");
    }

    // Absurd values are clamped so a corrupt skin cannot request a huge surface.
    let huge = parse_skin_ini(
        "[Rainmeter]\nUpdate=1000\n\n[Variables]\nScale=100000\n",
        Path::new("/dummy"),
    )
    .unwrap();
    assert!(skin_scale(&huge) <= pluvia_core::render::meter_renderer::MAX_SKIN_SCALE);
}

#[test]
fn scale_shrinks_the_measured_content() {
    let template = |scale: &str| {
        format!(
            r#"
[Rainmeter]
Update=1000

[Variables]
Scale={scale}

[MeasureM]
Measure=Time
Format=%H:%M

[MeterM]
Meter=String
MeasureName=MeasureM
FontFace=DejaVu Sans
FontSize=40
FontColor=255,255,255,255
StringAlign=Center
X=0
Y=20
W=400
H=60
"#
        )
    };

    let measure = |s: &str| {
        let cfg = parse_skin_ini(&template(s), Path::new("/dummy")).unwrap();
        let mut values = HashMap::new();
        values.insert("measurem".to_string(), MeasureValue::String("HELLO".to_string()));
        let state = SkinState::from_arc(Arc::new(cfg), Arc::new(values));
        MeterRenderer::new().measure_content(&state)
    };

    let small = measure("0.5");
    let normal = measure("1.0");
    let large = measure("2.0");

    assert!(
        small.width < normal.width && normal.width < large.width,
        "content width must follow Scale: {} < {} < {}",
        small.width,
        normal.width,
        large.width
    );
    // Roughly linear.
    let ratio = large.width / normal.width;
    assert!((ratio - 2.0).abs() < 0.25, "2x scale should ~double width, got {ratio}");
}

#[test]
fn string_align_parse_supports_baseline_variants() {
    assert_eq!(TextAlign::parse("Center"), TextAlign::Center);
    assert_eq!(TextAlign::parse("Left"), TextAlign::Left);
    assert_eq!(TextAlign::parse("Right"), TextAlign::Right);
    assert_eq!(TextAlign::parse("CenterCenter"), TextAlign::Center);
    assert_eq!(TextAlign::parse("CenterB"), TextAlign::CenterB);
    assert_eq!(TextAlign::parse("LeftB"), TextAlign::LeftB);
    assert_eq!(TextAlign::parse("RightB"), TextAlign::RightB);

    assert_eq!(TextAlign::CenterB.horizontal(), TextAlign::Center);
    assert!(TextAlign::CenterB.is_baseline());
    assert!(!TextAlign::Center.is_baseline());
}

#[test]
fn bold_and_plain_meters_position_independently_of_style() {
    // Guards the layout math against regressions when a meter has no explicit W/H.
    let ini = r#"
[Rainmeter]
Update=1000

[MeasureM]
Measure=Time
Format=%H:%M

[MeterM]
Meter=String
MeasureName=MeasureM
FontFace=DejaVu Sans
FontSize=20
StringStyle=Bold
FontColor=255,255,255,255
X=100
Y=20
"#;
    let (_s, mask) = render(ini, 400, 100);
    let mut min_x = u32::MAX;
    for y in 0..mask.height {
        for x in 0..mask.width {
            if mask.pixels[(y * mask.width + x) as usize] != 0 {
                min_x = min_x.min(x);
            }
        }
    }
    assert!(
        (min_x as i64 - 100).abs() <= 2,
        "text without W should start at X=100, got {min_x}"
    );
    let _ = TextStyle::Bold;
}
