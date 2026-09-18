use cairo::{Context, Format, ImageSurface};
use pluvia_core::ini::parse_skin_ini;
use pluvia_core::measures::MeasureValue;
use pluvia_core::render::hit_mask::AlphaHitMask;
use pluvia_core::render::meter_renderer::{MeterRenderer, SkinState};
use pluvia_core::render::pango_text::{
    add_application_font, PangoTextRenderer, TextAlign, TextCase, TextStyle,
};
use std::collections::HashMap;
use std::path::Path;
use tempfile::tempdir;

#[test]
fn test_pango_text_surface_rendering_and_hit_mask() {
    let surface = ImageSurface::create(Format::ARgb32, 400, 200).unwrap();
    let text_renderer = PangoTextRenderer::new();
    let rect = text_renderer
        .render_text(
            &surface,
            "MONDAY",
            "Sans Bold",
            32.0,
            (1.0, 1.0, 1.0, 1.0),
            50.0,
            50.0,
        )
        .unwrap();

    // Verify non-zero bounds
    assert!(rect.width > 0.0);
    assert!(rect.height > 0.0);

    let mask = AlphaHitMask::from_surface(&surface);
    // Non-transparent text area must register hit
    assert!(mask.contains(rect.x as i32 + 5, rect.y as i32 + 5));
    // Blank corner must be click-through
    assert!(!mask.contains(5, 5));
}

#[test]
fn test_alpha_hit_mask_bounding_rectangles_extraction() {
    let surface = ImageSurface::create(Format::ARgb32, 200, 200).unwrap();
    let cr = Context::new(&surface).unwrap();

    // Draw two solid boxes on transparent surface
    cr.set_source_rgba(1.0, 0.0, 0.0, 1.0);
    cr.rectangle(10.0, 10.0, 40.0, 30.0);
    cr.fill().unwrap();

    cr.set_source_rgba(0.0, 1.0, 0.0, 1.0);
    cr.rectangle(100.0, 120.0, 50.0, 40.0);
    cr.fill().unwrap();

    let mask = AlphaHitMask::from_surface(&surface);
    assert!(mask.contains(15, 15));
    assert!(mask.contains(110, 130));
    assert!(!mask.contains(70, 70));
    assert!(!mask.contains(0, 0));

    let rects = mask.to_rectangles();
    assert!(!rects.is_empty());

    // Hit test every extracted rectangle to ensure they only cover active pixels
    for r in &rects {
        let cx = (r.x + r.width / 2.0) as i32;
        let cy = (r.y + r.height / 2.0) as i32;
        assert!(
            mask.contains(cx, cy),
            "Extracted rectangle at ({}, {}) was not in hit mask",
            cx,
            cy
        );
    }
}

#[test]
fn test_fontconfig_add_application_font() {
    let valid_font = Path::new("/usr/share/fonts/Adwaita/AdwaitaMono-Regular.ttf");
    if valid_font.exists() {
        assert!(add_application_font(valid_font));
    }
    assert!(!add_application_font(Path::new("/nonexistent/font.ttf")));
}

#[test]
fn test_pango_text_alignment_case_and_styling() {
    assert_eq!(TextAlign::parse("Left"), TextAlign::Left);
    assert_eq!(TextAlign::parse("Center"), TextAlign::Center);
    assert_eq!(TextAlign::parse("Right"), TextAlign::Right);
    assert_eq!(TextAlign::parse("CenterCenter"), TextAlign::Center);

    assert_eq!(TextCase::Upper.apply("hello world"), "HELLO WORLD");
    assert_eq!(TextCase::Lower.apply("HELLO WORLD"), "hello world");
    assert_eq!(TextCase::Proper.apply("hello world"), "Hello World");

    assert_eq!(TextStyle::parse("Bold"), TextStyle::Bold);
    assert_eq!(TextStyle::parse("Italic"), TextStyle::Italic);
    assert_eq!(TextStyle::parse("BoldItalic"), TextStyle::BoldItalic);
    assert_eq!(TextStyle::parse("Normal"), TextStyle::Normal);
}

#[test]
fn test_meter_renderer_string_with_measure_substitution() {
    let ini = r#"
[Rainmeter]
Update=1000

[MeasureTime]
Measure=Time
Format=%H:%M

[MeterTime]
Meter=String
MeasureName=MeasureTime
FontFace=Sans
FontSize=24
FontColor=255,255,255,255
StringCase=Upper
StringStyle=Bold
Text="Time: %1"
X=10
Y=10
"#;

    let config = parse_skin_ini(ini, Path::new("/dummy")).unwrap();
    let mut measure_values = HashMap::new();
    measure_values.insert(
        "measuretime".to_string(),
        MeasureValue::String("14:30".to_string()),
    );

    let state = SkinState {
        config,
        measure_values,
    };

    let surface = ImageSurface::create(Format::ARgb32, 300, 100).unwrap();
    let renderer = MeterRenderer::new();
    let mask = renderer.render_to_surface(&state, &surface).unwrap();

    // Bounding mask should not be empty
    assert!(!mask.is_empty());
    let rects = mask.to_rectangles();
    assert!(!rects.is_empty());
}

#[test]
fn test_meter_renderer_bar_progress() {
    let ini = r#"
[Rainmeter]
Update=1000

[MeasureCpu]
Measure=Cpu

[MeterCpuBar]
Meter=Bar
MeasureName=MeasureCpu
BarColor=0,255,0,255
SolidColor=50,50,50,255
W=200
H=20
BarOrientation=Horizontal
X=0
Y=0
"#;

    let config = parse_skin_ini(ini, Path::new("/dummy")).unwrap();
    let mut measure_values = HashMap::new();
    // 50% CPU
    measure_values.insert("measurecpu".to_string(), MeasureValue::Number(50.0));

    let state = SkinState {
        config,
        measure_values,
    };

    let surface = ImageSurface::create(Format::ARgb32, 250, 50).unwrap();
    let renderer = MeterRenderer::new();
    let mask = renderer.render_to_surface(&state, &surface).unwrap();

    // The bar occupies 200x20
    assert!(mask.contains(50, 10)); // left half (active bar)
    assert!(mask.contains(150, 10)); // right half (solid background)
    assert!(!mask.contains(220, 10)); // beyond the bar W=200
}

#[test]
fn test_meter_renderer_image() {
    let tmp = tempdir().unwrap();
    let img_path = tmp.path().join("icon.png");

    // Create a 32x32 red PNG image using the `image` crate
    let img = image::RgbaImage::from_pixel(32, 32, image::Rgba([255, 0, 0, 255]));
    img.save(&img_path).unwrap();

    let ini = format!(
        r#"
[Rainmeter]
Update=1000

[MeterIcon]
Meter=Image
ImageName="{}"
W=64
H=64
PreserveAspectRatio=1
ImageAlpha=255
X=10
Y=10
"#,
        img_path.display()
    );

    let config = parse_skin_ini(&ini, tmp.path()).unwrap();
    let state = SkinState {
        config,
        measure_values: HashMap::new(),
    };

    let surface = ImageSurface::create(Format::ARgb32, 120, 120).unwrap();
    let renderer = MeterRenderer::new();
    let mask = renderer.render_to_surface(&state, &surface).unwrap();

    // Image scaled to 64x64 at (10, 10), so (40, 40) must be hit
    assert!(mask.contains(40, 40));
    // Blank outside (5, 5) and (100, 100) must be click-through
    assert!(!mask.contains(5, 5));
    assert!(!mask.contains(100, 100));
}

#[test]
fn test_meter_renderer_shape() {
    let ini = r#"
[Rainmeter]
Update=1000

[MeterBox]
Meter=Shape
Shape=Rectangle 20, 20, 100, 50 | Fill Color 0,0,255,255 | StrokeWidth 0
Shape2=Ellipse 160, 45, 20, 20 | Fill Color 255,0,0,255 | StrokeWidth 0
X=0
Y=0
"#;

    let config = parse_skin_ini(ini, Path::new("/dummy")).unwrap();
    let state = SkinState {
        config,
        measure_values: HashMap::new(),
    };

    let surface = ImageSurface::create(Format::ARgb32, 200, 100).unwrap();
    let renderer = MeterRenderer::new();
    let mask = renderer.render_to_surface(&state, &surface).unwrap();

    // Rectangle is 20..120 x, 20..70 y
    assert!(mask.contains(50, 40));
    // Ellipse center at (160, 45)
    assert!(mask.contains(160, 45));
    assert!(!mask.contains(5, 5));
    assert!(!mask.contains(195, 45));
}

#[test]
fn test_meter_renderer_roundline() {
    let ini = r#"
[Rainmeter]
Update=1000

[MeasureSec]
Measure=Time

[MeterDial]
Meter=Roundline
MeasureName=MeasureSec
LineLength=40
LineStart=10
Solid=1
StartAngle=0.0
RotationAngle=6.2831853
LineColor=255,255,255,255
X=50
Y=50
"#;

    let config = parse_skin_ini(ini, Path::new("/dummy")).unwrap();
    let mut measure_values = HashMap::new();
    measure_values.insert("measuresec".to_string(), MeasureValue::Number(0.5));

    let state = SkinState {
        config,
        measure_values,
    };

    let surface = ImageSurface::create(Format::ARgb32, 200, 200).unwrap();
    let renderer = MeterRenderer::new();
    let mask = renderer.render_to_surface(&state, &surface).unwrap();

    assert!(!mask.is_empty());
}

#[test]
fn test_meter_renderer_histogram() {
    let ini = r#"
[Rainmeter]
Update=1000

[MeasureCpu]
Measure=Cpu

[MeterHist]
Meter=Histogram
MeasureName=MeasureCpu
PrimaryColor=0,255,0,255
SolidColor=20,20,20,255
W=100
H=50
X=10
Y=10
"#;

    let config = parse_skin_ini(ini, Path::new("/dummy")).unwrap();
    let mut measure_values = HashMap::new();
    measure_values.insert("measurecpu".to_string(), MeasureValue::Number(40.0));

    let state = SkinState {
        config,
        measure_values,
    };

    let surface = ImageSurface::create(Format::ARgb32, 150, 80).unwrap();
    let renderer = MeterRenderer::new();
    let mask = renderer.render_to_surface(&state, &surface).unwrap();

    // Solid background of histogram should register
    assert!(mask.contains(50, 30));
    assert!(!mask.contains(5, 5));
}

#[test]
fn test_meter_renderer_relative_positioning() {
    let ini = r#"
[Rainmeter]
Update=1000

[MeterFirst]
Meter=Bar
SolidColor=255,0,0,255
W=50
H=30
X=10
Y=10

[MeterSecond]
Meter=Bar
SolidColor=0,255,0,255
W=40
H=30
X=10R
Y=0r
"#;

    let config = parse_skin_ini(ini, Path::new("/dummy")).unwrap();
    let state = SkinState {
        config,
        measure_values: HashMap::new(),
    };

    let surface = ImageSurface::create(Format::ARgb32, 150, 60).unwrap();
    let renderer = MeterRenderer::new();
    let mask = renderer.render_to_surface(&state, &surface).unwrap();

    // Meter1 is X=10..60, Y=10..40
    assert!(mask.contains(30, 20));
    // Gap is X=60..70
    assert!(!mask.contains(65, 20));
    // Meter2 is X=70..110, Y=10..40
    assert!(mask.contains(80, 20));
}

#[test]
fn test_meter_renderer_shape_path_and_round_rectangle() {
    let ini = r#"
[Rainmeter]
Update=1000

[MeterShapes]
Meter=Shape
Shape=RoundRectangle 10, 10, 80, 40, 6, 6 | Fill Color 255,0,0,255 | StrokeWidth 0
Shape2=Path 100, 10 | LineTo 160, 10 | LineTo 160, 60 | ClosePath 1 | Fill Color 0,255,0,255 | StrokeWidth 0
Shape3=Path "M 10 70 L 60 70 L 60 110 Z" | Fill Color 0,0,255,255 | StrokeWidth 0
X=0
Y=0
"#;

    let config = parse_skin_ini(ini, Path::new("/dummy")).unwrap();
    let state = SkinState {
        config,
        measure_values: HashMap::new(),
    };

    let surface = ImageSurface::create(Format::ARgb32, 200, 150).unwrap();
    let renderer = MeterRenderer::new();
    let mask = renderer.render_to_surface(&state, &surface).unwrap();

    // RoundRectangle inside at (50, 30)
    assert!(mask.contains(50, 30));
    // Rainmeter pipe Path inside at (130, 30)
    assert!(mask.contains(130, 30));
    // SVG Path inside at (35, 90)
    assert!(mask.contains(35, 90));

    // Outside bounds
    assert!(!mask.contains(5, 5));
    assert!(!mask.contains(90, 30));
    assert!(!mask.contains(180, 30));
}

#[test]
fn test_meter_renderer_webp_and_svg_images() {
    let tmp = tempdir().unwrap();

    // 1. Create a 32x32 WebP image
    let webp_path = tmp.path().join("test.webp");
    let img = image::RgbaImage::from_pixel(32, 32, image::Rgba([255, 128, 0, 255]));
    img.save(&webp_path).unwrap();

    // 2. Create an SVG file
    let svg_path = tmp.path().join("test.svg");
    let svg_content = r#"<svg width="40" height="40" xmlns="http://www.w3.org/2000/svg">
        <rect width="40" height="40" fill="purple" />
    </svg>"#;
    std::fs::write(&svg_path, svg_content).unwrap();

    let ini = format!(
        r#"
[Rainmeter]
Update=1000

[MeterWebp]
Meter=Image
ImageName="{}"
W=32
H=32
X=10
Y=10

[MeterSvg]
Meter=Image
ImageName="{}"
W=40
H=40
X=10R
Y=10
"#,
        webp_path.display(),
        svg_path.display()
    );

    let config = parse_skin_ini(&ini, tmp.path()).unwrap();
    let state = SkinState {
        config,
        measure_values: HashMap::new(),
    };

    let surface = ImageSurface::create(Format::ARgb32, 120, 70).unwrap();
    let renderer = MeterRenderer::new();
    let mask = renderer.render_to_surface(&state, &surface).unwrap();

    // WebP at (10, 10) size 32x32 -> (20, 20) hit
    assert!(mask.contains(20, 20));
    // SVG at (10+32+10=52, 10) size 40x40 -> (70, 25) hit
    assert!(mask.contains(70, 25));
    // Blank gaps
    assert!(!mask.contains(5, 5));
    assert!(!mask.contains(45, 20));
}

#[test]
fn test_image_cache_distinct_tints() {
    let tmp = tempdir().unwrap();
    let img_path = tmp.path().join("white_box.png");

    // Pure white 16x16 image
    let img = image::RgbaImage::from_pixel(16, 16, image::Rgba([255, 255, 255, 255]));
    img.save(&img_path).unwrap();

    let ini = format!(
        r#"
[Rainmeter]
Update=1000

[MeterRed]
Meter=Image
ImageName="{}"
ImageTint=255,0,0,255
W=16
H=16
X=0
Y=0

[MeterGreen]
Meter=Image
ImageName="{}"
ImageTint=0,255,0,255
W=16
H=16
X=30
Y=0
"#,
        img_path.display(),
        img_path.display()
    );

    let config = parse_skin_ini(&ini, tmp.path()).unwrap();
    let state = SkinState {
        config,
        measure_values: HashMap::new(),
    };

    let surface = ImageSurface::create(Format::ARgb32, 60, 30).unwrap();
    let renderer = MeterRenderer::new();
    let mask = renderer.render_to_surface(&state, &surface).unwrap();

    assert!(mask.contains(8, 8));
    assert!(mask.contains(38, 8));

    // Verify distinct colors in the surface data
    let stride = surface.stride() as usize;
    surface.flush();
    let data_ptr = unsafe { cairo::ffi::cairo_image_surface_get_data(surface.to_raw_none()) };
    assert!(!data_ptr.is_null());
    let data = unsafe { std::slice::from_raw_parts(data_ptr, 30 * stride) };

    // At (8, 8): Red meter -> [B=0, G=0, R=255, A=255]
    let red_offset = 8 * stride + 8 * 4;
    assert_eq!(data[red_offset], 0); // B
    assert_eq!(data[red_offset + 1], 0); // G
    assert_eq!(data[red_offset + 2], 255); // R
    assert_eq!(data[red_offset + 3], 255); // A

    // At (38, 8): Green meter -> [B=0, G=255, R=0, A=255]
    let green_offset = 8 * stride + 38 * 4;
    assert_eq!(data[green_offset], 0); // B
    assert_eq!(data[green_offset + 1], 255); // G
    assert_eq!(data[green_offset + 2], 0); // R
    assert_eq!(data[green_offset + 3], 255); // A
}

#[test]
fn test_text_rotation_origin_and_bounds() {
    let surface = ImageSurface::create(Format::ARgb32, 200, 200).unwrap();
    let cr = Context::new(&surface).unwrap();
    let text_renderer = PangoTextRenderer::new();

    // 1. Render normal horizontal text at (50, 50)
    let normal_rect = text_renderer
        .render_text_to_context(
            &cr,
            "TESTING ROTATION",
            "Sans",
            16.0,
            pluvia_core::render::Color::WHITE,
            50.0,
            50.0,
            TextAlign::Left,
            TextStyle::Normal,
            TextCase::None,
            true,
            0.0,
        )
        .unwrap();

    // Normal text is wider than tall
    assert!(normal_rect.width > normal_rect.height);
    assert!(normal_rect.x >= 50.0);
    assert!(normal_rect.y >= 50.0);

    // 2. Render 90-degree rotated text at (50, 50)
    let rot_surface = ImageSurface::create(Format::ARgb32, 200, 200).unwrap();
    let rot_cr = Context::new(&rot_surface).unwrap();
    let rot_rect = text_renderer
        .render_text_to_context(
            &rot_cr,
            "TESTING ROTATION",
            "Sans",
            16.0,
            pluvia_core::render::Color::WHITE,
            50.0,
            50.0,
            TextAlign::Left,
            TextStyle::Normal,
            TextCase::None,
            true,
            std::f64::consts::FRAC_PI_2, // 90 degrees
        )
        .unwrap();

    // Rotated 90 degrees: height is roughly original width, width is roughly original height
    assert!(rot_rect.height > rot_rect.width);
    // Bounding box must cover (50, 50)
    assert!(rot_rect.x <= 50.0);
    assert!(rot_rect.y <= 50.0);
    assert!(rot_rect.y + rot_rect.height >= 50.0 + normal_rect.width * 0.8);

    let mask = AlphaHitMask::from_surface(&rot_surface);
    assert!(!mask.is_empty());
}
