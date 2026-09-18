pub mod hit_mask;
pub mod meter_renderer;
pub mod pango_text;

pub use hit_mask::AlphaHitMask;
pub use meter_renderer::{MeterRenderer, RenderError, SkinState};
pub use pango_text::{add_application_font, PangoTextRenderer, TextAlign, TextCase, TextStyle};

/// 2D Rectangle with floating point coordinates and dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    };

    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn contains(&self, px: f64, py: f64) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }

    pub fn union(&self, other: &Rect) -> Rect {
        if self.width <= 0.0 && self.height <= 0.0 {
            return *other;
        }
        if other.width <= 0.0 && other.height <= 0.0 {
            return *self;
        }
        let x1 = self.x.min(other.x);
        let y1 = self.y.min(other.y);
        let x2 = (self.x + self.width).max(other.x + other.width);
        let y2 = (self.y + self.height).max(other.y + other.height);
        Rect {
            x: x1,
            y: y1,
            width: (x2 - x1).max(0.0),
            height: (y2 - y1).max(0.0),
        }
    }

    pub fn to_i32_tuple(&self) -> (i32, i32, i32, i32) {
        (
            self.x.round() as i32,
            self.y.round() as i32,
            self.width.round().max(0.0) as i32,
            self.height.round().max(0.0) as i32,
        )
    }
}

/// RGBA Color representation in normalized float space [0.0, 1.0].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

impl Color {
    pub const WHITE: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    pub const BLACK: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    pub const TRANSPARENT: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };

    pub fn rgba(r: f64, g: f64, b: f64, a: f64) -> Self {
        Self {
            r: r.clamp(0.0, 1.0),
            g: g.clamp(0.0, 1.0),
            b: b.clamp(0.0, 1.0),
            a: a.clamp(0.0, 1.0),
        }
    }

    pub fn rgb(r: f64, g: f64, b: f64) -> Self {
        Self::rgba(r, g, b, 1.0)
    }

    pub fn from_u8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self {
            r: r as f64 / 255.0,
            g: g as f64 / 255.0,
            b: b as f64 / 255.0,
            a: a as f64 / 255.0,
        }
    }

    /// Parse Rainmeter color string like "255, 255, 255", "255,255,255,128", "#FFFFFF", "#FFFFFF80".
    pub fn parse(s: &str) -> Option<Self> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            return None;
        }

        if let Some(hex) = trimmed.strip_prefix('#') {
            if hex.len() == 6 {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                return Some(Self::from_u8(r, g, b, 255));
            } else if hex.len() == 8 {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
                return Some(Self::from_u8(r, g, b, a));
            }
        }

        let parts: Vec<&str> = trimmed.split(',').map(str::trim).collect();
        if parts.len() == 3 || parts.len() == 4 {
            let r = parts[0].parse::<f64>().ok()?;
            let g = parts[1].parse::<f64>().ok()?;
            let b = parts[2].parse::<f64>().ok()?;
            let a = if parts.len() == 4 {
                parts[3].parse::<f64>().ok()?
            } else {
                255.0
            };
            return Some(Self {
                r: (r / 255.0).clamp(0.0, 1.0),
                g: (g / 255.0).clamp(0.0, 1.0),
                b: (b / 255.0).clamp(0.0, 1.0),
                a: (a / 255.0).clamp(0.0, 1.0),
            });
        }

        None
    }
}
