use crate::render::Rect;
use cairo::{Format, ImageSurface};

/// 1-bit alpha hit-test mask representing opaque and interactive regions.
#[derive(Debug, Clone, PartialEq)]
pub struct AlphaHitMask {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl AlphaHitMask {
    /// Creates an empty (transparent) hit mask.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; (width * height) as usize],
        }
    }

    /// Constructs an `AlphaHitMask` from a Cairo `ImageSurface` with alpha threshold >= 1.
    pub fn from_surface(surface: &ImageSurface) -> Self {
        Self::from_surface_with_threshold(surface, 1)
    }

    /// Constructs an `AlphaHitMask` from a Cairo `ImageSurface` with a custom alpha threshold.
    pub fn from_surface_with_threshold(surface: &ImageSurface, threshold: u8) -> Self {
        surface.flush();
        let width = surface.width() as u32;
        let height = surface.height() as u32;
        let format = surface.format();
        let stride = surface.stride() as usize;

        let mut mask = Self::new(width, height);

        if width == 0 || height == 0 {
            return mask;
        }

        let raw_ptr = surface.to_raw_none();
        let data_ptr = unsafe { cairo::ffi::cairo_image_surface_get_data(raw_ptr) };
        if data_ptr.is_null() {
            return mask;
        }
        let data = unsafe { std::slice::from_raw_parts(data_ptr, (height as usize) * stride) };

        match format {
            Format::ARgb32 => {
                for y in 0..height {
                    let row_start = y as usize * stride;
                    let mask_row_start = (y * width) as usize;
                    for x in 0..width {
                        let px_offset = row_start + x as usize * 4;
                        if px_offset + 3 < data.len() {
                            // In Cairo ARgb32 (little-endian: [B, G, R, A])
                            let alpha = data[px_offset + 3];
                            if alpha >= threshold {
                                mask.pixels[mask_row_start + x as usize] = 1;
                            }
                        }
                    }
                }
            }
            Format::A8 => {
                for y in 0..height {
                    let row_start = y as usize * stride;
                    let mask_row_start = (y * width) as usize;
                    for x in 0..width {
                        let px_offset = row_start + x as usize;
                        if px_offset < data.len() {
                            let alpha = data[px_offset];
                            if alpha >= threshold {
                                mask.pixels[mask_row_start + x as usize] = 1;
                            }
                        }
                    }
                }
            }
            Format::Rgb24 => {
                // Completely opaque surface
                mask.pixels.fill(1);
            }
            _ => {
                for y in 0..height {
                    let row_start = y as usize * stride;
                    let mask_row_start = (y * width) as usize;
                    for x in 0..width {
                        let px_offset = row_start + x as usize * 4;
                        if px_offset + 3 < data.len() {
                            let alpha = data[px_offset + 3];
                            if alpha >= threshold {
                                mask.pixels[mask_row_start + x as usize] = 1;
                            }
                        }
                    }
                }
            }
        }

        mask
    }

    /// Constructs an `AlphaHitMask` covering given bounding rectangles.
    pub fn from_rectangles(width: u32, height: u32, rects: &[Rect]) -> Self {
        let mut mask = Self::new(width, height);
        for r in rects {
            mask.add_rect(r);
        }
        mask
    }

    /// Marks a rectangular region as solid in the hit mask.
    pub fn add_rect(&mut self, rect: &Rect) {
        let x0 = rect.x.max(0.0) as u32;
        let y0 = rect.y.max(0.0) as u32;
        let x1 = ((rect.x + rect.width).ceil() as u32).min(self.width);
        let y1 = ((rect.y + rect.height).ceil() as u32).min(self.height);

        for y in y0..y1 {
            let row_start = (y * self.width) as usize;
            for x in x0..x1 {
                self.pixels[row_start + x as usize] = 1;
            }
        }
    }

    /// Tests if a point (x, y) hits an active/opaque pixel.
    pub fn contains(&self, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 {
            return false;
        }
        let ux = x as u32;
        let uy = y as u32;
        if ux >= self.width || uy >= self.height {
            return false;
        }
        self.pixels[(uy * self.width + ux) as usize] != 0
    }

    /// Returns true if no pixels in the mask are solid.
    pub fn is_empty(&self) -> bool {
        self.pixels.iter().all(|&p| p == 0)
    }

    /// Decomposes the 1-bit alpha mask into an optimal list of disjoint rectangles.
    /// Uses scanline run-length encoding coalesced vertically across consecutive rows.
    /// This format is directly submittable to Wayland `wl_surface.set_input_region`
    /// and X11 `XFixesSetWindowShapeRegion` so transparent areas are 100% click-through.
    pub fn to_rectangles(&self) -> Vec<Rect> {
        if self.width == 0 || self.height == 0 {
            return Vec::new();
        }

        // 1. Compute horizontal solid spans per scanline
        let mut row_spans: Vec<Vec<(u32, u32)>> = Vec::with_capacity(self.height as usize);
        for y in 0..self.height {
            let mut spans = Vec::new();
            let mut in_span = false;
            let mut start_x = 0;
            let row_offset = (y * self.width) as usize;

            for x in 0..self.width {
                let is_solid = self.pixels[row_offset + x as usize] != 0;
                if is_solid && !in_span {
                    in_span = true;
                    start_x = x;
                } else if !is_solid && in_span {
                    in_span = false;
                    spans.push((start_x, x - start_x));
                }
            }
            if in_span {
                spans.push((start_x, self.width - start_x));
            }
            row_spans.push(spans);
        }

        // 2. Coalesce identical spans vertically across consecutive rows
        let mut result = Vec::new();
        // active: (x, width, y_start, height)
        let mut active_rects: Vec<(u32, u32, u32, u32)> = Vec::new();

        for (y, current_spans) in row_spans.into_iter().enumerate() {
            let y = y as u32;
            let mut next_active = Vec::new();
            let mut unmatched_spans = current_spans;

            for (rx, rw, ry_start, rheight) in active_rects {
                if let Some(pos) = unmatched_spans
                    .iter()
                    .position(|&(sx, sw)| sx == rx && sw == rw)
                {
                    unmatched_spans.remove(pos);
                    next_active.push((rx, rw, ry_start, rheight + 1));
                } else {
                    result.push(Rect {
                        x: rx as f64,
                        y: ry_start as f64,
                        width: rw as f64,
                        height: rheight as f64,
                    });
                }
            }

            for (sx, sw) in unmatched_spans {
                next_active.push((sx, sw, y, 1));
            }

            active_rects = next_active;
        }

        for (rx, rw, ry_start, rheight) in active_rects {
            result.push(Rect {
                x: rx as f64,
                y: ry_start as f64,
                width: rw as f64,
                height: rheight as f64,
            });
        }

        result
    }
}
