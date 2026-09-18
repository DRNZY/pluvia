use crate::formulas::eval_formula;
use crate::ini::{MeterConfig, SkinConfig};
use crate::measures::MeasureValue;
use crate::render::hit_mask::AlphaHitMask;
use crate::render::pango_text::{PangoTextRenderer, TextAlign, TextCase, TextStyle};
use crate::render::{Color, Rect};
use crate::variables::VariableMap;
use crate::vfs::VfsResolver;
use cairo::{Context, Format, ImageSurface};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(thiserror::Error, Debug)]
pub enum RenderError {
    #[error("Cairo error: {0}")]
    Cairo(#[from] cairo::Error),
    #[error("Cairo borrow error: {0}")]
    CairoBorrow(#[from] cairo::BorrowError),
    #[error("Image error: {0}")]
    Image(String),
}

/// Dynamic snapshot of skin configuration and current measure values.
#[derive(Debug, Clone)]
pub struct SkinState {
    pub config: SkinConfig,
    pub measure_values: HashMap<String, MeasureValue>,
}

impl SkinState {
    pub fn new(config: SkinConfig, measure_values: HashMap<String, MeasureValue>) -> Self {
        Self {
            config,
            measure_values,
        }
    }

    pub fn get_measure_value(&self, name: &str) -> Option<&MeasureValue> {
        self.measure_values.get(&name.to_ascii_lowercase())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImageCacheKey {
    pub path: PathBuf,
    pub tint: Option<String>,
}

/// 2D Meter Renderer driven by Pango and Cairo.
pub struct MeterRenderer {
    text_renderer: PangoTextRenderer,
    vfs: VfsResolver,
    image_cache: Mutex<HashMap<ImageCacheKey, ImageSurface>>,
    histogram_history: Mutex<HashMap<String, VecDeque<f64>>>,
}

unsafe impl Send for MeterRenderer {}
unsafe impl Sync for MeterRenderer {}

impl Default for MeterRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl MeterRenderer {
    pub fn new() -> Self {
        Self {
            text_renderer: PangoTextRenderer::new(),
            vfs: VfsResolver::new(),
            image_cache: Mutex::new(HashMap::new()),
            histogram_history: Mutex::new(HashMap::new()),
        }
    }

    /// Renders all visible meters from `SkinState` onto the provided `ImageSurface`
    /// and returns the dynamic `AlphaHitMask` of non-transparent interactive areas.
    pub fn render_to_surface(
        &self,
        state: &SkinState,
        surface: &ImageSurface,
    ) -> Result<AlphaHitMask, RenderError> {
        let cr = Context::new(surface)?;

        let mut prev_x = 0.0;
        let mut prev_y = 0.0;
        let mut prev_w = 0.0;
        let mut prev_h = 0.0;

        for meter_name_lower in &state.config.meter_order {
            if let Some(meter) = state.config.meters.get(meter_name_lower) {
                if meter.hidden {
                    continue;
                }

                let x = resolve_coordinate(
                    meter.x.as_deref(),
                    prev_x,
                    prev_w,
                    &state.config.variables,
                );
                let y = resolve_coordinate(
                    meter.y.as_deref(),
                    prev_y,
                    prev_h,
                    &state.config.variables,
                );
                let w = meter.w.unwrap_or(0.0);
                let h = meter.h.unwrap_or(0.0);

                // Optional SolidColor background for meter box
                if let Some(sc) = meter.solid_color.as_deref().and_then(Color::parse) {
                    if w > 0.0 && h > 0.0 {
                        cr.set_source_rgba(sc.r, sc.g, sc.b, sc.a);
                        cr.rectangle(x, y, w, h);
                        cr.fill()?;
                    }
                }

                let m_type = meter.meter_type.to_ascii_lowercase();
                let rect = match m_type.as_str() {
                    "string" => self.render_string(&cr, meter, x, y, w, h, state)?,
                    "image" => self.render_image(&cr, meter, x, y, w, h, state)?,
                    "bar" => self.render_bar(&cr, meter, x, y, w, h, state)?,
                    "roundline" => self.render_roundline(&cr, meter, x, y, w, h, state)?,
                    "shape" => self.render_shape(&cr, meter, x, y)?,
                    "histogram" => self.render_histogram(&cr, meter, x, y, w, h, state)?,
                    _ => Rect::new(x, y, w, h),
                };

                prev_x = rect.x;
                prev_y = rect.y;
                prev_w = if w > 0.0 { w } else { rect.width };
                prev_h = if h > 0.0 { h } else { rect.height };
            }
        }

        Ok(AlphaHitMask::from_surface(surface))
    }

    fn render_string(
        &self,
        cr: &Context,
        meter: &MeterConfig,
        x: f64,
        y: f64,
        _w: f64,
        _h: f64,
        state: &SkinState,
    ) -> Result<Rect, RenderError> {
        let raw_text = meter.text.as_deref().unwrap_or("%1");
        let substituted = substitute_measures(raw_text, meter, state);

        let font_face = meter.font_face.as_deref().unwrap_or("Sans");
        let font_size = meter.font_size.unwrap_or(12.0);
        let color = meter
            .font_color
            .as_deref()
            .and_then(Color::parse)
            .unwrap_or(Color::WHITE);

        let align = meter
            .get("stringalign")
            .map(TextAlign::parse)
            .unwrap_or(TextAlign::Left);

        let style = meter
            .get("stringstyle")
            .map(TextStyle::parse)
            .unwrap_or(TextStyle::Normal);

        let case = meter
            .get("stringcase")
            .map(TextCase::parse)
            .unwrap_or(TextCase::None);

        let angle = meter
            .get("angle")
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);

        let rect = self.text_renderer.render_text_to_context(
            cr,
            &substituted,
            font_face,
            font_size,
            color,
            x,
            y,
            align,
            style,
            case,
            meter.anti_alias,
            angle,
        )?;

        Ok(rect)
    }

    fn render_image(
        &self,
        cr: &Context,
        meter: &MeterConfig,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        state: &SkinState,
    ) -> Result<Rect, RenderError> {
        let raw_img_name = meter
            .get("imagename")
            .or_else(|| {
                meter
                    .measure_name
                    .as_deref()
                    .and_then(|m| state.get_measure_value(m))
                    .and_then(|v| v.as_str())
            })
            .unwrap_or("");

        if raw_img_name.is_empty() {
            return Ok(Rect::new(x, y, w, h));
        }

        let resolved_path = self.resolve_image_path(raw_img_name, &state.config.skin_dir);
        let img_surface = match resolved_path {
            Some(ref path) => self.load_cairo_image(path, meter.get("imagetint"))?,
            None => return Ok(Rect::new(x, y, w, h)),
        };

        let src_w = img_surface.width() as f64;
        let src_h = img_surface.height() as f64;

        let target_w = if w > 0.0 { w } else { src_w };
        let target_h = if h > 0.0 { h } else { src_h };

        let preserve_aspect = meter
            .get("preserveaspectratio")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);

        let alpha = meter
            .get("imagealpha")
            .and_then(|s| s.parse::<f64>().ok())
            .map(|a| if a > 1.0 { a / 255.0 } else { a })
            .unwrap_or(1.0)
            .clamp(0.0, 1.0);

        cr.save()?;
        cr.rectangle(x, y, target_w, target_h);
        cr.clip();

        let (scale_x, scale_y, off_x, off_y) = match preserve_aspect {
            1 => {
                let s = (target_w / src_w).min(target_h / src_h);
                let ox = x + (target_w - src_w * s) / 2.0;
                let oy = y + (target_h - src_h * s) / 2.0;
                (s, s, ox, oy)
            }
            2 => {
                let s = (target_w / src_w).max(target_h / src_h);
                let ox = x + (target_w - src_w * s) / 2.0;
                let oy = y + (target_h - src_h * s) / 2.0;
                (s, s, ox, oy)
            }
            _ => {
                let sx = if src_w > 0.0 { target_w / src_w } else { 1.0 };
                let sy = if src_h > 0.0 { target_h / src_h } else { 1.0 };
                (sx, sy, x, y)
            }
        };

        cr.translate(off_x, off_y);
        cr.scale(scale_x, scale_y);
        cr.set_source_surface(&img_surface, 0.0, 0.0)?;

        if alpha < 1.0 {
            cr.paint_with_alpha(alpha)?;
        } else {
            cr.paint()?;
        }
        cr.restore()?;

        Ok(Rect::new(x, y, target_w, target_h))
    }

    fn render_bar(
        &self,
        cr: &Context,
        meter: &MeterConfig,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        state: &SkinState,
    ) -> Result<Rect, RenderError> {
        let val = meter
            .measure_name
            .as_deref()
            .and_then(|m| state.get_measure_value(m))
            .map(|v| v.to_number_val())
            .unwrap_or(0.0);

        let min_val = meter
            .get("minvalue")
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);
        let max_val = meter
            .get("maxvalue")
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(100.0);
        let orientation = meter
            .get("barorientation")
            .unwrap_or("horizontal")
            .to_ascii_lowercase();
        let flip = meter.get("flip").map(|s| s == "1").unwrap_or(false);

        let bar_color = meter
            .get("barcolor")
            .and_then(Color::parse)
            .unwrap_or(Color::rgba(0.0, 1.0, 0.0, 1.0));

        let progress = if max_val > min_val {
            ((val - min_val) / (max_val - min_val)).clamp(0.0, 1.0)
        } else {
            0.0
        };

        cr.set_source_rgba(bar_color.r, bar_color.g, bar_color.b, bar_color.a);
        if orientation == "vertical" {
            let bar_h = h * progress;
            let bar_y = if flip { y } else { y + h - bar_h };
            cr.rectangle(x, bar_y, w, bar_h);
            cr.fill()?;
        } else {
            let bar_w = w * progress;
            let bar_x = if flip { x + w - bar_w } else { x };
            cr.rectangle(bar_x, y, bar_w, h);
            cr.fill()?;
        }

        Ok(Rect::new(x, y, w, h))
    }

    fn render_roundline(
        &self,
        cr: &Context,
        meter: &MeterConfig,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        state: &SkinState,
    ) -> Result<Rect, RenderError> {
        let mut val = meter
            .measure_name
            .as_deref()
            .and_then(|m| state.get_measure_value(m))
            .map(|v| v.to_number_val())
            .unwrap_or(0.0);

        if let Some(rem_str) = meter.get("valueremainder") {
            if let Ok(rem) = rem_str.parse::<f64>() {
                if rem > 0.0 {
                    val %= rem;
                }
            }
        }

        let min_val = meter
            .get("minvalue")
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);
        let max_val = meter
            .get("maxvalue")
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(1.0);
        let progress = if max_val > min_val {
            ((val - min_val) / (max_val - min_val)).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let start_angle = meter
            .get("startangle")
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);
        let rotation_angle = meter
            .get("rotationangle")
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(std::f64::consts::TAU);
        let line_length = meter
            .get("linelength")
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or_else(|| w.max(h) / 2.0);
        let line_start = meter
            .get("linestart")
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);
        let line_width = meter
            .get("linewidth")
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(1.0);
        let solid = meter.get("solid").map(|s| s == "1").unwrap_or(false);
        let line_color = meter
            .get("linecolor")
            .and_then(Color::parse)
            .unwrap_or(Color::WHITE);

        let cx = x + if w > 0.0 { w / 2.0 } else { line_length };
        let cy = y + if h > 0.0 { h / 2.0 } else { line_length };

        cr.save()?;
        cr.set_source_rgba(line_color.r, line_color.g, line_color.b, line_color.a);

        if solid {
            let end_angle = start_angle + progress * rotation_angle;
            if rotation_angle >= 0.0 {
                cr.arc(cx, cy, line_length, start_angle, end_angle);
                if line_start > 0.0 {
                    cr.arc_negative(cx, cy, line_start, end_angle, start_angle);
                } else {
                    cr.line_to(cx, cy);
                }
            } else {
                cr.arc_negative(cx, cy, line_length, start_angle, end_angle);
                if line_start > 0.0 {
                    cr.arc(cx, cy, line_start, end_angle, start_angle);
                } else {
                    cr.line_to(cx, cy);
                }
            }
            cr.close_path();
            cr.fill()?;
        } else {
            let angle = start_angle + progress * rotation_angle;
            let x1 = cx + line_start * angle.cos();
            let y1 = cy + line_start * angle.sin();
            let x2 = cx + line_length * angle.cos();
            let y2 = cy + line_length * angle.sin();
            cr.set_line_width(line_width);
            cr.move_to(x1, y1);
            cr.line_to(x2, y2);
            cr.stroke()?;
        }

        cr.restore()?;

        let bound_size = line_length * 2.0;
        Ok(Rect::new(
            cx - line_length,
            cy - line_length,
            bound_size,
            bound_size,
        ))
    }

    fn render_shape(
        &self,
        cr: &Context,
        meter: &MeterConfig,
        base_x: f64,
        base_y: f64,
    ) -> Result<Rect, RenderError> {
        let mut shape_keys: Vec<(usize, String)> = Vec::new();
        for (k, v) in &meter.properties {
            if k == "shape" {
                shape_keys.push((1, v.clone()));
            } else if let Some(suffix) = k.strip_prefix("shape") {
                if let Ok(idx) = suffix.parse::<usize>() {
                    shape_keys.push((idx, v.clone()));
                }
            }
        }
        shape_keys.sort_by_key(|(idx, _)| *idx);

        let mut total_rect = Rect::new(base_x, base_y, 0.0, 0.0);

        for (_, shape_def) in shape_keys {
            if let Some(r) = self.render_single_shape(cr, &shape_def, base_x, base_y)? {
                total_rect = total_rect.union(&r);
            }
        }

        Ok(total_rect)
    }

    fn render_single_shape(
        &self,
        cr: &Context,
        def: &str,
        base_x: f64,
        base_y: f64,
    ) -> Result<Option<Rect>, RenderError> {
        let parts: Vec<&str> = def.split('|').map(str::trim).collect();
        if parts.is_empty() {
            return Ok(None);
        }

        let shape_decl = parts[0];
        let mut fill_color = Some(Color::WHITE);
        let mut stroke_color = None;
        let mut stroke_width = 0.0;

        for modifier in &parts[1..] {
            let lower = modifier.to_ascii_lowercase();
            if lower.starts_with("fill color") {
                let col_str = modifier["fill color".len()..].trim();
                fill_color = Color::parse(col_str);
            } else if lower.starts_with("fill none") {
                fill_color = None;
            } else if lower.starts_with("stroke color") {
                let col_str = modifier["stroke color".len()..].trim();
                stroke_color = Color::parse(col_str);
            } else if lower.starts_with("stroke none") {
                stroke_color = None;
            } else if lower.starts_with("strokewidth") {
                let num_str = modifier["strokewidth".len()..].trim();
                stroke_width = num_str.parse::<f64>().unwrap_or(1.0);
            }
        }

        let tokens: Vec<&str> = shape_decl
            .split(&[' ', ','][..])
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();

        if tokens.is_empty() {
            return Ok(None);
        }

        let kind = tokens[0].to_ascii_lowercase();
        let nums: Vec<f64> = tokens[1..]
            .iter()
            .filter_map(|s| s.parse::<f64>().ok())
            .collect();

        let mut bound = None;

        cr.save()?;
        cr.new_path();

        match kind.as_str() {
            "roundrectangle" | "rectangle" => {
                if nums.len() >= 4 {
                    let rx_val = nums[0] + base_x;
                    let ry_val = nums[1] + base_y;
                    let rw = nums[2];
                    let rh = nums[3];
                    let radius_x = if nums.len() >= 5 { nums[4] } else { 0.0 };
                    let radius_y = if nums.len() >= 6 {
                        nums[5]
                    } else {
                        radius_x
                    };

                    if radius_x > 0.0 && radius_y > 0.0 {
                        draw_rounded_rect(cr, rx_val, ry_val, rw, rh, radius_x, radius_y);
                    } else {
                        cr.rectangle(rx_val, ry_val, rw, rh);
                    }
                    bound = Some(Rect::new(rx_val, ry_val, rw, rh));
                }
            }
            "ellipse" => {
                if nums.len() >= 3 {
                    let cx = nums[0] + base_x;
                    let cy = nums[1] + base_y;
                    let rx_val = nums[2];
                    let ry_val = if nums.len() >= 4 { nums[3] } else { rx_val };

                    cr.save()?;
                    cr.translate(cx, cy);
                    if rx_val > 0.0 && ry_val > 0.0 {
                        cr.scale(1.0, ry_val / rx_val);
                        cr.arc(0.0, 0.0, rx_val, 0.0, std::f64::consts::TAU);
                    }
                    cr.restore()?;
                    bound = Some(Rect::new(
                        cx - rx_val,
                        cy - ry_val,
                        rx_val * 2.0,
                        ry_val * 2.0,
                    ));
                }
            }
            "line" => {
                if nums.len() >= 4 {
                    let x1 = nums[0] + base_x;
                    let y1 = nums[1] + base_y;
                    let x2 = nums[2] + base_x;
                    let y2 = nums[3] + base_y;
                    cr.move_to(x1, y1);
                    cr.line_to(x2, y2);
                    bound = Some(Rect::new(
                        x1.min(x2),
                        y1.min(y2),
                        (x2 - x1).abs(),
                        (y2 - y1).abs(),
                    ));
                }
            }
            "curve" => {
                if nums.len() >= 6 {
                    let x1 = nums[0] + base_x;
                    let y1 = nums[1] + base_y;
                    let x2 = nums[2] + base_x;
                    let y2 = nums[3] + base_y;
                    let cx1 = nums[4] + base_x;
                    let cy1 = nums[5] + base_y;
                    let (cx2, cy2) = if nums.len() >= 8 {
                        (nums[6] + base_x, nums[7] + base_y)
                    } else {
                        (cx1, cy1)
                    };
                    cr.move_to(x1, y1);
                    cr.curve_to(cx1, cy1, cx2, cy2, x2, y2);
                    bound = Some(Rect::new(
                        x1.min(x2).min(cx1).min(cx2),
                        y1.min(y2).min(cy1).min(cy2),
                        (x2 - x1).abs().max((cx1 - x1).abs()),
                        (y2 - y1).abs().max((cy1 - y1).abs()),
                    ));
                }
            }
            "path" => {
                let raw_args = shape_decl
                    .strip_prefix("path")
                    .or_else(|| shape_decl.strip_prefix("Path"))
                    .unwrap_or("")
                    .trim();
                let unquoted = raw_args.trim_matches('"').trim();
                let has_svg = unquoted
                    .chars()
                    .any(|c| matches!(c, 'M' | 'm' | 'L' | 'l' | 'C' | 'c' | 'Z' | 'z'));

                if has_svg {
                    execute_svg_path(cr, unquoted, base_x, base_y)?;
                } else {
                    let start_nums: Vec<f64> = unquoted
                        .split(&[' ', ','][..])
                        .filter_map(|s| s.parse::<f64>().ok())
                        .collect();
                    if start_nums.len() >= 2 {
                        cr.move_to(base_x + start_nums[0], base_y + start_nums[1]);
                    }
                }

                for part in &parts[1..] {
                    let p_trimmed = part.trim();
                    let p_lower = p_trimmed.to_ascii_lowercase();
                    if p_lower.starts_with("lineto") {
                        let line_nums: Vec<f64> = p_trimmed["lineto".len()..]
                            .split(&[' ', ','][..])
                            .filter_map(|s| s.parse::<f64>().ok())
                            .collect();
                        if line_nums.len() >= 2 {
                            cr.line_to(base_x + line_nums[0], base_y + line_nums[1]);
                        }
                    } else if p_lower.starts_with("curveto") {
                        let curve_nums: Vec<f64> = p_trimmed["curveto".len()..]
                            .split(&[' ', ','][..])
                            .filter_map(|s| s.parse::<f64>().ok())
                            .collect();
                        if curve_nums.len() >= 6 {
                            cr.curve_to(
                                base_x + curve_nums[2],
                                base_y + curve_nums[3],
                                base_x + curve_nums[4],
                                base_y + curve_nums[5],
                                base_x + curve_nums[0],
                                base_y + curve_nums[1],
                            );
                        } else if curve_nums.len() >= 4 {
                            cr.curve_to(
                                base_x + curve_nums[2],
                                base_y + curve_nums[3],
                                base_x + curve_nums[2],
                                base_y + curve_nums[3],
                                base_x + curve_nums[0],
                                base_y + curve_nums[1],
                            );
                        }
                    } else if p_lower.starts_with("closepath") || p_lower == "close" {
                        cr.close_path();
                    }
                }

                if let Ok((x1, y1, x2, y2)) = cr.path_extents() {
                    bound = Some(Rect::new(x1, y1, (x2 - x1).max(0.0), (y2 - y1).max(0.0)));
                }
            }
            _ => {}
        }

        if let Some(fill) = fill_color {
            cr.set_source_rgba(fill.r, fill.g, fill.b, fill.a);
            if stroke_color.is_some() && stroke_width > 0.0 {
                cr.fill_preserve()?;
            } else {
                cr.fill()?;
            }
        }

        if let Some(stroke) = stroke_color {
            if stroke_width > 0.0 {
                cr.set_source_rgba(stroke.r, stroke.g, stroke.b, stroke.a);
                cr.set_line_width(stroke_width);
                cr.stroke()?;
            }
        }

        cr.restore()?;
        Ok(bound)
    }

    fn render_histogram(
        &self,
        cr: &Context,
        meter: &MeterConfig,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        state: &SkinState,
    ) -> Result<Rect, RenderError> {
        let primary_val = meter
            .measure_name
            .as_deref()
            .and_then(|m| state.get_measure_value(m))
            .map(|v| v.to_number_val())
            .unwrap_or(0.0);

        let primary_color = meter
            .get("primarycolor")
            .and_then(Color::parse)
            .unwrap_or(Color::rgba(0.0, 1.0, 0.0, 0.8));

        let max_points = (w.ceil() as usize).max(10);
        let mut history_guard = self.histogram_history.lock().unwrap();
        let history = history_guard
            .entry(meter.name.to_ascii_lowercase())
            .or_insert_with(|| VecDeque::with_capacity(max_points));
        history.push_back(primary_val);
        while history.len() > max_points {
            history.pop_front();
        }

        let min_val = meter
            .get("minvalue")
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);
        let autoscale = meter.get("autoscale").map(|s| s == "1").unwrap_or(false);
        let max_val = if autoscale {
            history.iter().copied().fold(1.0f64, f64::max)
        } else {
            meter
                .get("maxvalue")
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(100.0)
        };

        if history.len() >= 2 && max_val > min_val {
            cr.save()?;
            cr.set_source_rgba(
                primary_color.r,
                primary_color.g,
                primary_color.b,
                primary_color.a,
            );

            let step = w / (max_points - 1).max(1) as f64;
            let start_x = x + w - (history.len() - 1) as f64 * step;

            cr.move_to(start_x, y + h);
            for (i, &v) in history.iter().enumerate() {
                let px = start_x + i as f64 * step;
                let ratio = ((v - min_val) / (max_val - min_val)).clamp(0.0, 1.0);
                let py = y + h - ratio * h;
                cr.line_to(px, py);
            }
            let last_x = start_x + (history.len() - 1) as f64 * step;
            cr.line_to(last_x, y + h);
            cr.close_path();
            cr.fill()?;
            cr.restore()?;
        }

        Ok(Rect::new(x, y, w, h))
    }

    fn resolve_image_path(&self, raw: &str, skin_dir: &Path) -> Option<PathBuf> {
        let p = Path::new(raw);
        if p.is_absolute() && p.exists() {
            return Some(p.to_path_buf());
        }
        self.vfs.resolve(skin_dir, raw)
    }

    fn load_cairo_image(
        &self,
        path: &Path,
        tint: Option<&str>,
    ) -> Result<ImageSurface, RenderError> {
        let cache_key = ImageCacheKey {
            path: path.to_path_buf(),
            tint: tint.map(str::to_string),
        };

        let mut cache = self.image_cache.lock().unwrap();
        if let Some(surf) = cache.get(&cache_key) {
            return Ok(surf.clone());
        }

        let is_svg = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("svg"))
            .unwrap_or(false);

        let (w, h, rgba_bytes) = if is_svg {
            let svg_data = std::fs::read(path).map_err(|e| RenderError::Image(e.to_string()))?;
            let opt = resvg::usvg::Options::default();
            let tree = resvg::usvg::Tree::from_data(&svg_data, &opt)
                .map_err(|e| RenderError::Image(e.to_string()))?;
            let pixmap_size = tree.size().to_int_size();
            let mut pixmap = resvg::tiny_skia::Pixmap::new(pixmap_size.width(), pixmap_size.height())
                .ok_or_else(|| RenderError::Image("Failed to allocate SVG pixmap".to_string()))?;
            resvg::render(&tree, resvg::tiny_skia::Transform::default(), &mut pixmap.as_mut());
            (
                pixmap_size.width(),
                pixmap_size.height(),
                pixmap.take(),
            )
        } else {
            let img = image::open(path).map_err(|e| RenderError::Image(e.to_string()))?;
            let rgba = img.to_rgba8();
            let (img_w, img_h) = rgba.dimensions();
            (img_w, img_h, rgba.into_raw())
        };

        let tint_col = tint.and_then(Color::parse);

        let mut surface = ImageSurface::create(Format::ARgb32, w as i32, h as i32)?;
        let stride = surface.stride() as usize;
        {
            let mut data = surface.data()?;

            for y in 0..h {
                let row_offset = y as usize * stride;
                let src_row_offset = (y * w) as usize * 4;
                for x in 0..w {
                    let px_idx = src_row_offset + x as usize * 4;
                    let mut r = rgba_bytes[px_idx] as f64 / 255.0;
                    let mut g = rgba_bytes[px_idx + 1] as f64 / 255.0;
                    let mut b = rgba_bytes[px_idx + 2] as f64 / 255.0;
                    let mut a = rgba_bytes[px_idx + 3] as f64 / 255.0;

                    if let Some(t) = tint_col {
                        r *= t.r;
                        g *= t.g;
                        b *= t.b;
                        a *= t.a;
                    }

                    // Premultiplied ARgb32
                    let pr = (r * a * 255.0).round() as u8;
                    let pg = (g * a * 255.0).round() as u8;
                    let pb = (b * a * 255.0).round() as u8;
                    let pa = (a * 255.0).round() as u8;

                    let out_offset = row_offset + x as usize * 4;
                    data[out_offset] = pb;
                    data[out_offset + 1] = pg;
                    data[out_offset + 2] = pr;
                    data[out_offset + 3] = pa;
                }
            }
        }

        cache.insert(cache_key, surface.clone());
        Ok(surface)
    }
}

fn execute_svg_path(
    cr: &Context,
    path_str: &str,
    base_x: f64,
    base_y: f64,
) -> Result<(), RenderError> {
    let mut normalized = String::with_capacity(path_str.len() * 2);
    let mut prev_char = ' ';
    for ch in path_str.chars() {
        if ch.is_ascii_alphabetic() {
            normalized.push(' ');
            normalized.push(ch);
            normalized.push(' ');
        } else if ch == ',' {
            normalized.push(' ');
        } else if ch == '-' && prev_char != 'e' && prev_char != 'E' {
            normalized.push(' ');
            normalized.push('-');
        } else {
            normalized.push(ch);
        }
        prev_char = ch;
    }

    let tokens: Vec<&str> = normalized.split_whitespace().collect();
    let mut i = 0;
    let mut current_cmd = ' ';
    let mut cur_x = base_x;
    let mut cur_y = base_y;

    while i < tokens.len() {
        let tok = tokens[i];
        if tok.len() == 1 && tok.chars().next().unwrap().is_ascii_alphabetic() {
            current_cmd = tok.chars().next().unwrap();
            i += 1;
        }

        match current_cmd {
            'M' => {
                if i + 1 < tokens.len() {
                    let x = tokens[i].parse::<f64>().unwrap_or(0.0);
                    let y = tokens[i + 1].parse::<f64>().unwrap_or(0.0);
                    cur_x = base_x + x;
                    cur_y = base_y + y;
                    cr.move_to(cur_x, cur_y);
                    i += 2;
                    current_cmd = 'L';
                } else {
                    break;
                }
            }
            'm' => {
                if i + 1 < tokens.len() {
                    let dx = tokens[i].parse::<f64>().unwrap_or(0.0);
                    let dy = tokens[i + 1].parse::<f64>().unwrap_or(0.0);
                    cur_x += dx;
                    cur_y += dy;
                    cr.move_to(cur_x, cur_y);
                    i += 2;
                    current_cmd = 'l';
                } else {
                    break;
                }
            }
            'L' => {
                if i + 1 < tokens.len() {
                    let x = tokens[i].parse::<f64>().unwrap_or(0.0);
                    let y = tokens[i + 1].parse::<f64>().unwrap_or(0.0);
                    cur_x = base_x + x;
                    cur_y = base_y + y;
                    cr.line_to(cur_x, cur_y);
                    i += 2;
                } else {
                    break;
                }
            }
            'l' => {
                if i + 1 < tokens.len() {
                    let dx = tokens[i].parse::<f64>().unwrap_or(0.0);
                    let dy = tokens[i + 1].parse::<f64>().unwrap_or(0.0);
                    cur_x += dx;
                    cur_y += dy;
                    cr.line_to(cur_x, cur_y);
                    i += 2;
                } else {
                    break;
                }
            }
            'H' => {
                if i < tokens.len() {
                    let x = tokens[i].parse::<f64>().unwrap_or(0.0);
                    cur_x = base_x + x;
                    cr.line_to(cur_x, cur_y);
                    i += 1;
                } else {
                    break;
                }
            }
            'h' => {
                if i < tokens.len() {
                    let dx = tokens[i].parse::<f64>().unwrap_or(0.0);
                    cur_x += dx;
                    cr.line_to(cur_x, cur_y);
                    i += 1;
                } else {
                    break;
                }
            }
            'V' => {
                if i < tokens.len() {
                    let y = tokens[i].parse::<f64>().unwrap_or(0.0);
                    cur_y = base_y + y;
                    cr.line_to(cur_x, cur_y);
                    i += 1;
                } else {
                    break;
                }
            }
            'v' => {
                if i < tokens.len() {
                    let dy = tokens[i].parse::<f64>().unwrap_or(0.0);
                    cur_y += dy;
                    cr.line_to(cur_x, cur_y);
                    i += 1;
                } else {
                    break;
                }
            }
            'C' => {
                if i + 5 < tokens.len() {
                    let cx1 = base_x + tokens[i].parse::<f64>().unwrap_or(0.0);
                    let cy1 = base_y + tokens[i + 1].parse::<f64>().unwrap_or(0.0);
                    let cx2 = base_x + tokens[i + 2].parse::<f64>().unwrap_or(0.0);
                    let cy2 = base_y + tokens[i + 3].parse::<f64>().unwrap_or(0.0);
                    let x = base_x + tokens[i + 4].parse::<f64>().unwrap_or(0.0);
                    let y = base_y + tokens[i + 5].parse::<f64>().unwrap_or(0.0);
                    cur_x = x;
                    cur_y = y;
                    cr.curve_to(cx1, cy1, cx2, cy2, cur_x, cur_y);
                    i += 6;
                } else {
                    break;
                }
            }
            'c' => {
                if i + 5 < tokens.len() {
                    let cx1 = cur_x + tokens[i].parse::<f64>().unwrap_or(0.0);
                    let cy1 = cur_y + tokens[i + 1].parse::<f64>().unwrap_or(0.0);
                    let cx2 = cur_x + tokens[i + 2].parse::<f64>().unwrap_or(0.0);
                    let cy2 = cur_y + tokens[i + 3].parse::<f64>().unwrap_or(0.0);
                    let x = cur_x + tokens[i + 4].parse::<f64>().unwrap_or(0.0);
                    let y = cur_y + tokens[i + 5].parse::<f64>().unwrap_or(0.0);
                    cur_x = x;
                    cur_y = y;
                    cr.curve_to(cx1, cy1, cx2, cy2, cur_x, cur_y);
                    i += 6;
                } else {
                    break;
                }
            }
            'Z' | 'z' => {
                cr.close_path();
            }
            _ => {
                i += 1;
            }
        }
    }

    Ok(())
}

fn draw_rounded_rect(
    cr: &Context,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    radius_x: f64,
    radius_y: f64,
) {
    let pi = std::f64::consts::PI;
    let r_x = radius_x.min(w / 2.0);
    let r_y = radius_y.min(h / 2.0);

    cr.save().unwrap();
    cr.translate(x, y);
    cr.scale(1.0, r_y / r_x);

    let h_scaled = h * (r_x / r_y);

    cr.new_sub_path();
    cr.arc(w - r_x, r_x, r_x, -pi / 2.0, 0.0);
    cr.arc(w - r_x, h_scaled - r_x, r_x, 0.0, pi / 2.0);
    cr.arc(r_x, h_scaled - r_x, r_x, pi / 2.0, pi);
    cr.arc(r_x, r_x, r_x, pi, 3.0 * pi / 2.0);
    cr.close_path();

    cr.restore().unwrap();
}

fn resolve_coordinate(
    raw: Option<&str>,
    prev_pos: f64,
    prev_size: f64,
    vars: &VariableMap,
) -> f64 {
    let raw = match raw {
        Some(s) => s.trim(),
        None => return 0.0,
    };
    if raw.is_empty() {
        return 0.0;
    }

    if let Some(rest) = raw.strip_suffix('R') {
        let val = eval_coord_num(rest, vars);
        prev_pos + prev_size + val
    } else if let Some(rest) = raw.strip_suffix('r') {
        let val = eval_coord_num(rest, vars);
        prev_pos + val
    } else {
        eval_coord_num(raw, vars)
    }
}

fn eval_coord_num(s: &str, vars: &VariableMap) -> f64 {
    let exp = vars.expand(s);
    if let Ok(val) = eval_formula(&exp, vars) {
        val
    } else {
        exp.trim().parse::<f64>().unwrap_or(0.0)
    }
}

fn substitute_measures(template: &str, meter: &MeterConfig, state: &SkinState) -> String {
    let mut result = template.to_string();
    for (idx, mname) in meter.measure_names.iter().enumerate() {
        let val_str = state
            .get_measure_value(mname)
            .map(|v| v.to_string_val())
            .unwrap_or_default();
        let placeholder = format!("%{}", idx + 1);
        result = result.replace(&placeholder, &val_str);
    }
    if result.contains("%0") {
        let val0 = meter
            .measure_names
            .first()
            .and_then(|m| state.get_measure_value(m))
            .map(|v| v.to_string_val())
            .unwrap_or_default();
        result = result.replace("%0", &val0);
    }
    result
}
