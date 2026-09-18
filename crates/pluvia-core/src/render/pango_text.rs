use crate::render::{Color, Rect};
use cairo::{Context, ImageSurface};
use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

#[link(name = "fontconfig")]
extern "C" {
    fn FcConfigGetCurrent() -> *mut std::ffi::c_void;
    fn FcConfigAppFontAddFile(
        config: *mut std::ffi::c_void,
        file: *const libc::c_char,
    ) -> libc::c_int;
}

/// Dynamic application font registration for `@Resources/Fonts/*.otf|ttf`.
pub fn add_application_font(path: &Path) -> bool {
    if !path.exists() {
        return false;
    }
    let c_path = match CString::new(path.as_os_str().as_bytes()) {
        Ok(s) => s,
        Err(_) => return false,
    };
    unsafe {
        let config = FcConfigGetCurrent();
        if config.is_null() {
            return false;
        }
        let res = FcConfigAppFontAddFile(config, c_path.as_ptr());
        res != 0
    }
}

/// Text alignment within layout and meter bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

impl TextAlign {
    pub fn parse(s: &str) -> Self {
        let lower = s.to_ascii_lowercase();
        if lower.starts_with("center") {
            TextAlign::Center
        } else if lower.starts_with("right") {
            TextAlign::Right
        } else {
            TextAlign::Left
        }
    }
}

/// Text case transformation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextCase {
    #[default]
    None,
    Upper,
    Lower,
    Proper,
}

impl TextCase {
    pub fn parse(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "upper" => TextCase::Upper,
            "lower" => TextCase::Lower,
            "proper" => TextCase::Proper,
            _ => TextCase::None,
        }
    }

    pub fn apply(&self, s: &str) -> String {
        match self {
            TextCase::None => s.to_string(),
            TextCase::Upper => s.to_uppercase(),
            TextCase::Lower => s.to_lowercase(),
            TextCase::Proper => {
                let mut result = String::with_capacity(s.len());
                let mut capitalize_next = true;
                for c in s.chars() {
                    if c.is_alphanumeric() {
                        if capitalize_next {
                            result.extend(c.to_uppercase());
                            capitalize_next = false;
                        } else {
                            result.extend(c.to_lowercase());
                        }
                    } else {
                        capitalize_next = true;
                        result.push(c);
                    }
                }
                result
            }
        }
    }
}

/// Text styling (weight and slant).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextStyle {
    #[default]
    Normal,
    Bold,
    Italic,
    BoldItalic,
}

impl TextStyle {
    pub fn parse(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "bold" => TextStyle::Bold,
            "italic" => TextStyle::Italic,
            "bolditalic" => TextStyle::BoldItalic,
            _ => TextStyle::Normal,
        }
    }
}

/// PangoCairo text layout and rasterization engine.
#[derive(Debug, Clone, Default)]
pub struct PangoTextRenderer;

impl PangoTextRenderer {
    pub fn new() -> Self {
        Self
    }

    /// Renders text with basic styling to an `ImageSurface` and returns the ink bounding box.
    pub fn render_text(
        &self,
        surface: &ImageSurface,
        text: &str,
        font_face: &str,
        font_size: f64,
        color: (f64, f64, f64, f64),
        x: f64,
        y: f64,
    ) -> Result<Rect, cairo::Error> {
        let cr = Context::new(surface)?;
        self.render_text_to_context(
            &cr,
            text,
            font_face,
            font_size,
            Color::rgba(color.0, color.1, color.2, color.3),
            x,
            y,
            TextAlign::Left,
            TextStyle::Normal,
            TextCase::None,
            true,
            0.0,
        )
    }

    /// Renders text to a Cairo `Context` with full alignment, casing, styling, and rotation support.
    pub fn render_text_to_context(
        &self,
        cr: &Context,
        text: &str,
        font_face: &str,
        font_size: f64,
        color: Color,
        x: f64,
        y: f64,
        align: TextAlign,
        style: TextStyle,
        case: TextCase,
        antialias: bool,
        angle: f64,
    ) -> Result<Rect, cairo::Error> {
        let formatted_text = case.apply(text);

        cr.save()?;

        if antialias {
            cr.set_antialias(cairo::Antialias::Subpixel);
        } else {
            cr.set_antialias(cairo::Antialias::None);
        }

        let layout = pangocairo::functions::create_layout(cr);
        layout.set_text(&formatted_text);

        let mut desc = pango::FontDescription::from_string(font_face);
        // Rainmeter FontSize in points -> Pango units
        desc.set_size((font_size.max(1.0) * pango::SCALE as f64) as i32);

        match style {
            TextStyle::Bold => desc.set_weight(pango::Weight::Bold),
            TextStyle::Italic => desc.set_style(pango::Style::Italic),
            TextStyle::BoldItalic => {
                desc.set_weight(pango::Weight::Bold);
                desc.set_style(pango::Style::Italic);
            }
            TextStyle::Normal => {}
        }

        layout.set_font_description(Some(&desc));

        match align {
            TextAlign::Left => layout.set_alignment(pango::Alignment::Left),
            TextAlign::Center => layout.set_alignment(pango::Alignment::Center),
            TextAlign::Right => layout.set_alignment(pango::Alignment::Right),
        }

        let (ink_rect, logical_rect) = layout.pixel_extents();
        let text_w = logical_rect.width() as f64;
        let text_h = logical_rect.height() as f64;

        let render_x = match align {
            TextAlign::Left => x,
            TextAlign::Center => x - text_w / 2.0,
            TextAlign::Right => x - text_w,
        };
        let render_y = y;

        cr.move_to(render_x, render_y);

        if angle != 0.0 {
            cr.rotate(angle);
        }

        cr.set_source_rgba(color.r, color.g, color.b, color.a);
        pangocairo::functions::show_layout(cr, &layout);

        cr.restore()?;

        let bound_x = render_x + ink_rect.x() as f64;
        let bound_y = render_y + ink_rect.y() as f64;
        let bound_w = (ink_rect.width() as f64).max(text_w);
        let bound_h = (ink_rect.height() as f64).max(text_h);

        Ok(Rect {
            x: bound_x,
            y: bound_y,
            width: bound_w,
            height: bound_h,
        })
    }
}
