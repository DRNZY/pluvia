use crate::formulas::eval_formula;
use crate::ini::{MeterConfig, SkinConfig};
use crate::measures::MeasureValue;
use crate::render::hit_mask::AlphaHitMask;
use crate::render::pango_text::{PangoTextRenderer, TextAlign, TextCase, TextStyle};
use crate::render::{Color, Rect};
use crate::variables::VariableMap;
use crate::vfs::VfsResolver;
use cairo::{Context, Format, ImageSurface};
use std::collections::{HashMap, HashSet, VecDeque};
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
    pub config: std::sync::Arc<SkinConfig>,
    pub measure_values: std::sync::Arc<HashMap<String, MeasureValue>>,
}

impl SkinState {
    pub fn new(config: SkinConfig, measure_values: HashMap<String, MeasureValue>) -> Self {
        Self {
            config: std::sync::Arc::new(config),
            measure_values: std::sync::Arc::new(measure_values),
        }
    }

    pub fn from_arc(
        config: std::sync::Arc<SkinConfig>,
        measure_values: std::sync::Arc<HashMap<String, MeasureValue>>,
    ) -> Self {
        Self { config, measure_values }
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

        // Collect all meters referenced as a Container
        let mut container_meters = HashSet::new();
        for meter in state.config.meters.values() {
            if let Some(c) = meter.properties.get("container") {
                container_meters.insert(c.trim().to_ascii_lowercase());
            }
        }

        for meter_name_lower in &state.config.meter_order {
            if container_meters.contains(meter_name_lower) {
                continue;
            }

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
                let w = meter.w.or_else(|| {
                    meter.get("w").and_then(|s| {
                        let exp = state.config.variables.expand_with_context(s, Some(&meter.name), Some(&state.measure_values));
                        eval_formula(&exp, &state.config.variables).ok().or_else(|| exp.trim().parse::<f64>().ok())
                    })
                }).unwrap_or(0.0);
                let h = meter.h.or_else(|| {
                    meter.get("h").and_then(|s| {
                        let exp = state.config.variables.expand_with_context(s, Some(&meter.name), Some(&state.measure_values));
                        eval_formula(&exp, &state.config.variables).ok().or_else(|| exp.trim().parse::<f64>().ok())
                    })
                }).unwrap_or(0.0);

                let pad = meter
                    .properties
                    .get("padding")
                    .map(|p| parse_padding(p, &state.config.variables))
                    .unwrap_or((0.0, 0.0, 0.0, 0.0));
                let render_x = x + pad.0;
                let render_y = y + pad.1;

                // Optional SolidColor background for meter box
                if let Some(sc) = meter.solid_color.as_deref().and_then(Color::parse) {
                    if w > 0.0 && h > 0.0 {
                        cr.set_source_rgba(sc.r, sc.g, sc.b, sc.a);
                        cr.rectangle(render_x, render_y, w, h);
                        cr.fill()?;
                    }
                }

                let m_type = meter.meter_type.to_ascii_lowercase();
                let rect = match m_type.as_str() {
                    "string" => self.render_string(&cr, meter, render_x, render_y, w, h, state)?,
                    "image" => self.render_image(&cr, meter, render_x, render_y, w, h, state)?,
                    "bar" => self.render_bar(&cr, meter, render_x, render_y, w, h, state)?,
                    "roundline" => self.render_roundline(&cr, meter, render_x, render_y, w, h, state)?,
                    "shape" => self.render_shape(&cr, meter, render_x, render_y, state)?,
                    "histogram" => self.render_histogram(&cr, meter, render_x, render_y, w, h, state)?,
                    "rotator" => self.render_rotator(&cr, meter, render_x, render_y, w, h, state)?,
                    "bitmap" => self.render_bitmap(&cr, meter, render_x, render_y, w, h, state)?,
                    "line" => self.render_line(&cr, meter, render_x, render_y, w, h, state)?,
                    _ => Rect::new(render_x, render_y, w, h),
                };

                let effective_rect = Rect::new(
                    rect.x - pad.0,
                    rect.y - pad.1,
                    rect.width + pad.0 + pad.2,
                    rect.height + pad.1 + pad.3,
                );

                prev_x = effective_rect.x;
                prev_y = effective_rect.y;
                prev_w = if w > 0.0 { w + pad.0 + pad.2 } else { effective_rect.width };
                prev_h = if h > 0.0 { h + pad.1 + pad.3 } else { effective_rect.height };
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
        let raw_text = meter.text.as_deref().or_else(|| meter.get("text")).unwrap_or("%1");
        let substituted = substitute_measures(raw_text, meter, state);
        let final_text = state.config.variables.expand_with_context(
            &substituted,
            Some(&meter.name),
            Some(&state.measure_values),
        );

        let raw_font = meter.font_face.as_deref().or_else(|| meter.get("fontface")).unwrap_or("Sans");
        let exp_font = state.config.variables.expand_with_context(
            raw_font,
            Some(&meter.name),
            Some(&state.measure_values),
        );
        let font_face = if exp_font.is_empty() || exp_font.starts_with('#') {
            "Sans"
        } else {
            &exp_font
        };

        let font_size = meter.font_size.or_else(|| {
            meter.get("fontsize").and_then(|s| {
                let exp = state.config.variables.expand_with_context(s, Some(&meter.name), Some(&state.measure_values));
                eval_formula(&exp, &state.config.variables).ok().or_else(|| exp.trim().parse::<f64>().ok())
            })
        }).unwrap_or(12.0).max(1.0);

        let raw_color = meter.font_color.as_deref().or_else(|| meter.get("fontcolor")).unwrap_or("255,255,255,255");
        let exp_color = state.config.variables.expand_with_context(
            raw_color,
            Some(&meter.name),
            Some(&state.measure_values),
        );
        let color = Color::parse(&exp_color).unwrap_or(Color::WHITE);

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

        let mut letter_spacing = 0.0;
        if let Some(spacing_str) = meter.get("characterspacing").or_else(|| meter.get("tracking")) {
            if let Ok(val) = spacing_str.trim().parse::<f64>() {
                letter_spacing = val;
            }
        }
        if letter_spacing == 0.0 {
            for (key, val) in &meter.properties {
                if key.starts_with("inlinesetting") {
                    let parts: Vec<&str> = val.split('|').map(|s| s.trim()).collect();
                    if parts.len() >= 2 && parts[0].eq_ignore_ascii_case("characterspacing") {
                        let s1 = parts[1].parse::<f64>().unwrap_or(0.0);
                        let s2 = if parts.len() >= 3 {
                            parts[2].parse::<f64>().unwrap_or(0.0)
                        } else {
                            s1
                        };
                        letter_spacing = s1 + s2;
                        break;
                    }
                }
            }
        }

        let rect = self.text_renderer.render_text_to_context_with_spacing(
            cr,
            &final_text,
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
            letter_spacing,
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

        let exp_img_name = state.config.variables.expand_with_context(
            raw_img_name,
            Some(&meter.name),
            Some(&state.measure_values),
        );
        let resolved_path = self.resolve_image_path(&exp_img_name, &state.config.skin_dir);
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

        let (min_val, max_val) = resolve_meter_range(meter, state, val, 1.0);
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

        let (min_val, max_val) = resolve_meter_range(meter, state, val, 1.0);
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
        state: &SkinState,
    ) -> Result<Rect, RenderError> {
        let mut shape_keys: Vec<(usize, String, String)> = Vec::new();
        for (k, v) in &meter.properties {
            let k_lower = k.to_ascii_lowercase();
            if k_lower == "shape" {
                shape_keys.push((1, "shape".to_string(), v.clone()));
            } else if let Some(suffix) = k_lower.strip_prefix("shape") {
                if let Ok(idx) = suffix.parse::<usize>() {
                    shape_keys.push((idx, k_lower.clone(), v.clone()));
                }
            }
        }
        shape_keys.sort_by_key(|(idx, _, _)| *idx);

        // Find which shapes are referenced by Combine or Union/Intersect/Exclude modifiers
        let mut combined_keys = HashSet::new();
        for (_, _, shape_def) in &shape_keys {
            let parts: Vec<&str> = shape_def.split('|').map(str::trim).collect();
            if let Some(first) = parts.first() {
                let first_lower = first.to_ascii_lowercase();
                if first_lower.starts_with("combine") {
                    let combined_target = first["combine".len()..].trim().to_ascii_lowercase();
                    combined_keys.insert(combined_target);
                    for modifier in &parts[1..] {
                        let m_lower = modifier.to_ascii_lowercase();
                        for op in &["union", "intersect", "xor", "exclude"] {
                            if m_lower.starts_with(op) {
                                let sub_target = modifier[op.len()..].trim().to_ascii_lowercase();
                                combined_keys.insert(sub_target);
                            }
                        }
                    }
                }
            }
        }

        let mut total_rect = Rect::new(base_x, base_y, 0.0, 0.0);

        for (_, key_name, shape_def) in &shape_keys {
            if combined_keys.contains(key_name) {
                continue;
            }

            if let Some(r) = self.render_single_shape(cr, meter, shape_def, base_x, base_y, state)? {
                total_rect = total_rect.union(&r);
            }
        }

        Ok(total_rect)
    }

    fn render_single_shape(
        &self,
        cr: &Context,
        meter: &MeterConfig,
        def: &str,
        base_x: f64,
        base_y: f64,
        state: &SkinState,
    ) -> Result<Option<Rect>, RenderError> {
        let parts: Vec<&str> = def.split('|').map(str::trim).collect();
        if parts.is_empty() {
            return Ok(None);
        }

        let vars = &state.config.variables;
        let measures = &state.measure_values;

        let shape_decl = parts[0];
        let mut fill_color = Some(Color::WHITE);
        let mut stroke_color = None;
        let mut stroke_width = 0.0;
        let mut rotate: Option<(f64, f64, f64)> = None;

        for modifier in &parts[1..] {
            let lower = modifier.to_ascii_lowercase();
            if lower.starts_with("fill color") {
                let col_str = modifier["fill color".len()..].trim();
                let expanded = vars.expand_with_context(col_str, Some(&meter.name), Some(measures));
                fill_color = Color::parse(&expanded);
            } else if lower.starts_with("fill none") {
                fill_color = None;
            } else if lower.starts_with("stroke color") {
                let col_str = modifier["stroke color".len()..].trim();
                let expanded = vars.expand_with_context(col_str, Some(&meter.name), Some(measures));
                stroke_color = Color::parse(&expanded);
            } else if lower.starts_with("stroke none") {
                stroke_color = None;
            } else if lower.starts_with("strokewidth") {
                let num_str = modifier["strokewidth".len()..].trim();
                let expanded = vars.expand_with_context(num_str, Some(&meter.name), Some(measures));
                stroke_width = eval_formula(&expanded, vars)
                    .unwrap_or_else(|_| expanded.parse::<f64>().unwrap_or(1.0));
            } else if lower.starts_with("rotate") {
                let rest = modifier["rotate".len()..].trim();
                let (_, rot_nums) = tokenize_shape_declaration(&format!("dummy {}", rest), vars, Some(&meter.name), Some(measures));
                if !rot_nums.is_empty() {
                    let angle = rot_nums[0];
                    let cx = if rot_nums.len() >= 2 { rot_nums[1] } else { base_x };
                    let cy = if rot_nums.len() >= 3 { rot_nums[2] } else { base_y };
                    rotate = Some((angle, cx, cy));
                }
            }
        }

        cr.save()?;
        if let Some((angle_deg, cx, cy)) = rotate {
            let rad = angle_deg.to_radians();
            cr.translate(cx, cy);
            cr.rotate(rad);
            cr.translate(-cx, -cy);
        }

        cr.new_path();
        let mut bound = None;

        let lower_decl = shape_decl.to_ascii_lowercase();
        if lower_decl.starts_with("combine") {
            let mut sub_shapes = Vec::new();
            let base_target = shape_decl["combine".len()..].trim();
            sub_shapes.push(base_target);
            for modifier in &parts[1..] {
                let m_lower = modifier.to_ascii_lowercase();
                for op in &["union", "intersect", "xor", "exclude"] {
                    if m_lower.starts_with(op) {
                        let sub_target = modifier[op.len()..].trim();
                        sub_shapes.push(sub_target);
                    }
                }
            }

            for sub_name in sub_shapes {
                if let Some(sub_def) = meter.properties.get(&sub_name.to_ascii_lowercase()) {
                    let sub_parts: Vec<&str> = sub_def.split('|').map(str::trim).collect();
                    if !sub_parts.is_empty() {
                        let sub_decl = sub_parts[0];
                        let (kind, nums) = tokenize_shape_declaration(sub_decl, vars, Some(&meter.name), Some(measures));
                        self.append_shape_path(cr, meter, state, &kind, &nums, base_x, base_y, sub_decl, &sub_parts[1..])?;

                        // Inherit sub_shape color/stroke if parent combine didn't override
                        if fill_color.is_none() || fill_color == Some(Color::WHITE) {
                            for smod in &sub_parts[1..] {
                                let sm_lower = smod.to_ascii_lowercase();
                                if sm_lower.starts_with("fill color") {
                                    let col = smod["fill color".len()..].trim();
                                    let exp = vars.expand_with_context(col, Some(&meter.name), Some(measures));
                                    if let Some(c) = Color::parse(&exp) {
                                        fill_color = Some(c);
                                    }
                                } else if sm_lower.starts_with("fill none") {
                                    fill_color = None;
                                } else if sm_lower.starts_with("stroke color") {
                                    let col = smod["stroke color".len()..].trim();
                                    let exp = vars.expand_with_context(col, Some(&meter.name), Some(measures));
                                    if let Some(c) = Color::parse(&exp) {
                                        stroke_color = Some(c);
                                    }
                                } else if sm_lower.starts_with("strokewidth") {
                                    let num_str = smod["strokewidth".len()..].trim();
                                    let exp = vars.expand_with_context(num_str, Some(&meter.name), Some(measures));
                                    if let Ok(w) = eval_formula(&exp, vars) {
                                        stroke_width = w;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if let Ok((x1, y1, x2, y2)) = cr.path_extents() {
                bound = Some(Rect::new(x1, y1, (x2 - x1).max(0.0), (y2 - y1).max(0.0)));
            }
        } else {
            let (kind, nums) = tokenize_shape_declaration(shape_decl, vars, Some(&meter.name), Some(measures));
            bound = self.append_shape_path(cr, meter, state, &kind, &nums, base_x, base_y, shape_decl, &parts[1..])?;
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

    fn append_shape_path(
        &self,
        cr: &Context,
        meter: &MeterConfig,
        state: &SkinState,
        kind: &str,
        nums: &[f64],
        base_x: f64,
        base_y: f64,
        shape_decl: &str,
        modifiers: &[&str],
    ) -> Result<Option<Rect>, RenderError> {
        let mut bound = None;
        match kind {
            "roundrectangle" | "rectangle" => {
                if nums.len() >= 4 {
                    let rx_val = nums[0] + base_x;
                    let ry_val = nums[1] + base_y;
                    let rw = nums[2];
                    let rh = nums[3];
                    let radius_x = if nums.len() >= 5 { nums[4] } else { 0.0 };
                    let radius_y = if nums.len() >= 6 { nums[5] } else { radius_x };

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

                // Check if unquoted matches a named custom Path property on the meter (e.g. Area, Line)
                if let Some(path_prop_val) = meter.properties.get(&unquoted.to_ascii_lowercase()) {
                    let exp_path_prop = state.config.variables.expand_with_context(
                        path_prop_val,
                        Some(&meter.name),
                        Some(&state.measure_values),
                    );
                    let segs: Vec<&str> = exp_path_prop.split('|').map(str::trim).collect();
                    if !segs.is_empty() {
                        let start_nums = parse_point_nums(
                            segs[0],
                            &state.config.variables,
                            Some(&meter.name),
                            Some(&state.measure_values),
                        );
                        if start_nums.len() >= 2 {
                            cr.move_to(base_x + start_nums[0], base_y + start_nums[1]);
                        }
                        for seg in &segs[1..] {
                            let s_lower = seg.to_ascii_lowercase();
                            if s_lower.starts_with("lineto") {
                                let rest = seg["lineto".len()..].trim();
                                let line_nums = parse_point_nums(
                                    rest,
                                    &state.config.variables,
                                    Some(&meter.name),
                                    Some(&state.measure_values),
                                );
                                if line_nums.len() >= 2 {
                                    cr.line_to(base_x + line_nums[0], base_y + line_nums[1]);
                                }
                            } else if s_lower.starts_with("curveto") {
                                let rest = seg["curveto".len()..].trim();
                                let curve_nums = parse_point_nums(
                                    rest,
                                    &state.config.variables,
                                    Some(&meter.name),
                                    Some(&state.measure_values),
                                );
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
                            } else if s_lower.starts_with("closepath") || s_lower == "close" {
                                cr.close_path();
                            }
                        }
                    }
                } else {
                    let has_svg = unquoted
                        .chars()
                        .any(|c| matches!(c, 'M' | 'm' | 'L' | 'l' | 'C' | 'c' | 'Z' | 'z'));

                    if has_svg {
                        execute_svg_path(cr, unquoted, base_x, base_y)?;
                    } else {
                        let start_nums = parse_point_nums(
                            unquoted,
                            &state.config.variables,
                            Some(&meter.name),
                            Some(&state.measure_values),
                        );
                        if start_nums.len() >= 2 {
                            cr.move_to(base_x + start_nums[0], base_y + start_nums[1]);
                        }
                    }
                }

                for part in modifiers {
                    let p_trimmed = part.trim();
                    let p_lower = p_trimmed.to_ascii_lowercase();
                    if p_lower.starts_with("lineto") {
                        let line_nums = parse_point_nums(
                            &p_trimmed["lineto".len()..],
                            &state.config.variables,
                            Some(&meter.name),
                            Some(&state.measure_values),
                        );
                        if line_nums.len() >= 2 {
                            cr.line_to(base_x + line_nums[0], base_y + line_nums[1]);
                        }
                    } else if p_lower.starts_with("curveto") {
                        let curve_nums = parse_point_nums(
                            &p_trimmed["curveto".len()..],
                            &state.config.variables,
                            Some(&meter.name),
                            Some(&state.measure_values),
                        );
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

        let autoscale = meter.get("autoscale").map(|s| s == "1").unwrap_or(false);
        let (min_val, max_val) = if autoscale {
            (0.0, history.iter().copied().fold(1.0f64, f64::max))
        } else {
            resolve_meter_range(meter, state, primary_val, 100.0)
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

    fn render_rotator(
        &self,
        cr: &Context,
        meter: &MeterConfig,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        state: &SkinState,
    ) -> Result<Rect, RenderError> {
        let image_raw = meter
            .get("imagename")
            .or_else(|| meter.get("image"))
            .unwrap_or("");

        let image_path = self.resolve_image_path(image_raw, &state.config.skin_dir);
        let img = if let Some(p) = image_path {
            self.load_cairo_image(&p, meter.get("imagetint"))?
        } else {
            return Ok(Rect::new(x, y, w, h));
        };

        let img_w = img.width() as f64;
        let img_h = img.height() as f64;

        let offset_x = meter
            .get("offsetx")
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(img_w / 2.0);
        let offset_y = meter
            .get("offsety")
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(img_h / 2.0);

        let start_angle = meter
            .get("startangle")
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);
        let rotation_angle = meter
            .get("rotationangle")
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(std::f64::consts::TAU);

        let val = meter
            .measure_name
            .as_deref()
            .and_then(|m| state.get_measure_value(m))
            .map(|v| v.to_number_val())
            .unwrap_or(0.0);

        let max_val = meter
            .get("valueremainder")
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(1.0);

        let norm_val = if max_val != 0.0 { (val % max_val) / max_val } else { 0.0 };
        let angle = start_angle + norm_val * rotation_angle;

        cr.save()?;
        cr.translate(x + offset_x, y + offset_y);
        cr.rotate(angle);
        cr.set_source_surface(&img, -offset_x, -offset_y)?;
        cr.paint()?;
        cr.restore()?;

        Ok(Rect::new(x, y, img_w.max(w), img_h.max(h)))
    }

    fn render_bitmap(
        &self,
        cr: &Context,
        meter: &MeterConfig,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        state: &SkinState,
    ) -> Result<Rect, RenderError> {
        let image_raw = meter
            .get("bitmapimage")
            .or_else(|| meter.get("imagename"))
            .unwrap_or("");

        let image_path = self.resolve_image_path(image_raw, &state.config.skin_dir);
        let img = if let Some(p) = image_path {
            self.load_cairo_image(&p, meter.get("imagetint"))?
        } else {
            return Ok(Rect::new(x, y, w, h));
        };

        let frames = meter
            .get("bitmapframes")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(1)
            .max(1);

        let zero_frame = meter
            .get("bitmapzeroframe")
            .map(|s| s == "1" || s.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        let val = meter
            .measure_name
            .as_deref()
            .and_then(|m| state.get_measure_value(m))
            .map(|v| v.to_number_val())
            .unwrap_or(0.0);

        let (min_val, max_val) = resolve_meter_range(meter, state, val, 1.0);
        let progress = if max_val > min_val {
            ((val - min_val) / (max_val - min_val)).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let frame_idx = if frames <= 1 {
            0
        } else if zero_frame {
            if val <= min_val || progress <= 0.0 {
                0
            } else {
                let active_frames = frames.saturating_sub(1);
                1 + ((progress * (active_frames - 1) as f64).round() as usize).min(active_frames - 1)
            }
        } else {
            ((progress * (frames - 1) as f64).round() as usize).min(frames - 1)
        };

        let img_w = img.width() as f64;
        let img_h = img.height() as f64;

        let (frame_w, frame_h, src_x, src_y) = if img_w > img_h {
            let fw = img_w / frames as f64;
            (fw, img_h, frame_idx as f64 * fw, 0.0)
        } else {
            let fh = img_h / frames as f64;
            (img_w, fh, 0.0, frame_idx as f64 * fh)
        };

        cr.save()?;
        cr.rectangle(x, y, frame_w, frame_h);
        cr.clip();
        cr.set_source_surface(&img, x - src_x, y - src_y)?;
        cr.paint()?;
        cr.restore()?;

        Ok(Rect::new(x, y, frame_w.max(w), frame_h.max(h)))
    }

    fn render_line(
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

        let color = meter
            .get("linecolor")
            .and_then(Color::parse)
            .unwrap_or(Color::WHITE);

        let line_width = meter
            .get("linewidth")
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(1.0);

        let mut map = self.histogram_history.lock().unwrap();
        let history = map.entry(meter.name.to_ascii_lowercase()).or_default();
        history.push_back(val);
        while history.len() > 100 {
            history.pop_front();
        }

        if history.len() >= 2 && w > 0.0 && h > 0.0 {
            cr.save()?;
            cr.set_source_rgba(color.r, color.g, color.b, color.a);
            cr.set_line_width(line_width);

            let (_min_val, max_val) = resolve_meter_range(meter, state, val, 100.0);
            let max_val = max_val.max(1.0);

            let step = (w / 100.0).max(1.0);
            let start_x = x + w - (history.len() - 1) as f64 * step;

            let first_val = history[0];
            let first_y = y + h - ((first_val / max_val).clamp(0.0, 1.0) * h);
            cr.move_to(start_x, first_y);

            for (i, &v) in history.iter().enumerate().skip(1) {
                let px = start_x + i as f64 * step;
                let py = y + h - ((v / max_val).clamp(0.0, 1.0) * h);
                cr.line_to(px, py);
            }

            cr.stroke()?;
            cr.restore()?;
        }

        Ok(Rect::new(x, y, w, h))
    }

    fn resolve_image_path(&self, raw: &str, skin_dir: &Path) -> Option<PathBuf> {
        let p = Path::new(raw);
        if p.is_absolute() && p.exists() {
            return Some(p.to_path_buf());
        }
        let norm = VfsResolver::normalize_rel_path(raw);
        let mut curr = Some(skin_dir);
        while let Some(dir) = curr {
            if let Some(path) = self.vfs.resolve(dir, &norm) {
                return Some(path);
            }
            let direct = dir.join(&norm);
            if direct.exists() {
                return Some(direct);
            }
            curr = dir.parent();
        }
        None
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
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let r_x = radius_x.min(w / 2.0).max(0.0);
    let r_y = radius_y.min(h / 2.0).max(0.0);

    if r_x <= 0.0 || r_y <= 0.0 {
        cr.rectangle(x, y, w, h);
        return;
    }

    let pi = std::f64::consts::PI;

    if (r_x - r_y).abs() < 1e-6 {
        cr.new_sub_path();
        cr.arc(x + w - r_x, y + r_x, r_x, -pi / 2.0, 0.0);
        cr.arc(x + w - r_x, y + h - r_x, r_x, 0.0, pi / 2.0);
        cr.arc(x + r_x, y + h - r_x, r_x, pi / 2.0, pi);
        cr.arc(x + r_x, y + r_x, r_x, pi, 3.0 * pi / 2.0);
        cr.close_path();
        return;
    }

    let _ = cr.save();
    cr.translate(x, y);
    cr.scale(1.0, r_y / r_x);

    let h_scaled = h * (r_x / r_y);

    cr.new_sub_path();
    cr.arc(w - r_x, r_x, r_x, -pi / 2.0, 0.0);
    cr.arc(w - r_x, h_scaled - r_x, r_x, 0.0, pi / 2.0);
    cr.arc(r_x, h_scaled - r_x, r_x, pi / 2.0, pi);
    cr.arc(r_x, r_x, r_x, pi, 3.0 * pi / 2.0);
    cr.close_path();

    let _ = cr.restore();
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

fn tokenize_shape_declaration(
    decl: &str,
    vars: &VariableMap,
    current_section: Option<&str>,
    measures: Option<&HashMap<String, MeasureValue>>,
) -> (String, Vec<f64>) {
    let trimmed = decl.trim();
    if trimmed.is_empty() {
        return (String::new(), Vec::new());
    }
    let mut chars = trimmed.char_indices().peekable();
    let mut kind_end = trimmed.len();
    while let Some(&(i, ch)) = chars.peek() {
        if ch.is_whitespace() || ch == ',' || ch == '(' {
            kind_end = i;
            break;
        }
        chars.next();
    }
    let kind = trimmed[..kind_end].trim().to_ascii_lowercase();

    let mut nums = Vec::new();
    let rest = &trimmed[kind_end..];
    let mut i = 0;
    let bytes = rest.as_bytes();
    while i < bytes.len() {
        while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b',' || bytes[i] == b'\t') {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        if bytes[i] == b'(' {
            let start = i;
            let mut depth = 0;
            while i < bytes.len() {
                if bytes[i] == b'(' {
                    depth += 1;
                } else if bytes[i] == b')' {
                    depth -= 1;
                    if depth == 0 {
                        i += 1;
                        break;
                    }
                }
                i += 1;
            }
            let formula = &rest[start..i];
            let expanded = if let Some(m) = measures {
                vars.expand_with_context(formula, current_section, Some(m))
            } else {
                vars.expand(formula)
            };
            let val = eval_formula(&expanded, vars).unwrap_or(0.0);
            nums.push(val);
        } else {
            let start = i;
            while i < bytes.len()
                && bytes[i] != b' '
                && bytes[i] != b','
                && bytes[i] != b'\t'
                && bytes[i] != b'('
            {
                i += 1;
            }
            let token = &rest[start..i];
            let expanded = if let Some(m) = measures {
                vars.expand_with_context(token, current_section, Some(m))
            } else {
                vars.expand(token)
            };
            if let Ok(v) = eval_formula(&expanded, vars) {
                nums.push(v);
            } else if let Ok(v) = expanded.trim().parse::<f64>() {
                nums.push(v);
            }
        }
    }
    (kind, nums)
}

fn parse_padding(raw: &str, vars: &VariableMap) -> (f64, f64, f64, f64) {
    let (_, nums) = tokenize_shape_declaration(&format!("dummy {}", raw), vars, None, None);
    if nums.len() >= 4 {
        (nums[0], nums[1], nums[2], nums[3])
    } else if nums.len() == 1 {
        (nums[0], nums[0], nums[0], nums[0])
    } else {
        (0.0, 0.0, 0.0, 0.0)
    }
}

/// Interactive bounding box for a meter containing mouse action strings.
#[derive(Debug, Clone, PartialEq)]
pub struct MeterHitBox {
    pub meter_name: String,
    pub rect: Rect,
    pub left_mouse_up_action: Option<String>,
    pub left_mouse_down_action: Option<String>,
    pub left_mouse_double_click_action: Option<String>,
    pub right_mouse_up_action: Option<String>,
    pub middle_mouse_up_action: Option<String>,
    pub mouse_over_action: Option<String>,
    pub mouse_leave_action: Option<String>,
    pub mouse_scroll_up_action: Option<String>,
    pub mouse_scroll_down_action: Option<String>,
}

impl MeterHitBox {
    /// Returns true if the coordinate (x, y) lies inside this meter's bounding box.
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.rect.x
            && x <= self.rect.x + self.rect.width
            && y >= self.rect.y
            && y <= self.rect.y + self.rect.height
    }
}

impl MeterRenderer {
    /// Computes interactive hit boxes with associated mouse action bangs for all rendered meters.
    pub fn get_interactive_hit_boxes(&self, state: &SkinState) -> Vec<MeterHitBox> {
        let mut hit_boxes = Vec::new();
        let mut prev_x = 0.0;
        let mut prev_y = 0.0;
        let mut prev_w = 0.0;
        let mut prev_h = 0.0;

        let mut container_meters = HashSet::new();
        for meter in state.config.meters.values() {
            if let Some(c) = meter.properties.get("container") {
                container_meters.insert(c.trim().to_ascii_lowercase());
            }
        }

        for meter_name_lower in &state.config.meter_order {
            if container_meters.contains(meter_name_lower) {
                continue;
            }

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
                let w = meter.w.or_else(|| {
                    meter.get("w").and_then(|s| {
                        let exp = state.config.variables.expand_with_context(s, Some(&meter.name), Some(&state.measure_values));
                        eval_formula(&exp, &state.config.variables).ok().or_else(|| exp.trim().parse::<f64>().ok())
                    })
                }).unwrap_or(0.0);
                let h = meter.h.or_else(|| {
                    meter.get("h").and_then(|s| {
                        let exp = state.config.variables.expand_with_context(s, Some(&meter.name), Some(&state.measure_values));
                        eval_formula(&exp, &state.config.variables).ok().or_else(|| exp.trim().parse::<f64>().ok())
                    })
                }).unwrap_or(0.0);

                let pad = meter
                    .properties
                    .get("padding")
                    .map(|p| parse_padding(p, &state.config.variables))
                    .unwrap_or((0.0, 0.0, 0.0, 0.0));

                let eff_w = if w > 0.0 { w } else { 50.0 };
                let eff_h = if h > 0.0 { h } else { 20.0 };

                let effective_rect = Rect::new(
                    x,
                    y,
                    eff_w + pad.0 + pad.2,
                    eff_h + pad.1 + pad.3,
                );

                prev_x = effective_rect.x;
                prev_y = effective_rect.y;
                prev_w = effective_rect.width;
                prev_h = effective_rect.height;

                let left_up = meter.get("leftmouseupaction").map(str::to_string);
                let left_down = meter.get("leftmousedownaction").map(str::to_string);
                let left_dbl = meter.get("leftmousedoubleclickaction").map(str::to_string);
                let right_up = meter.get("rightmouseupaction").map(str::to_string);
                let middle_up = meter.get("middlemouseupaction").map(str::to_string);
                let over = meter.get("mouseoveraction").map(str::to_string);
                let leave = meter.get("mouseleaveaction").map(str::to_string);
                let scroll_up = meter.get("mousescrollupaction").map(str::to_string);
                let scroll_down = meter.get("mousescrolldownaction").map(str::to_string);

                if left_up.is_some()
                    || left_down.is_some()
                    || left_dbl.is_some()
                    || right_up.is_some()
                    || middle_up.is_some()
                    || over.is_some()
                    || leave.is_some()
                    || scroll_up.is_some()
                    || scroll_down.is_some()
                {
                    hit_boxes.push(MeterHitBox {
                        meter_name: meter.name.clone(),
                        rect: effective_rect,
                        left_mouse_up_action: left_up,
                        left_mouse_down_action: left_down,
                        left_mouse_double_click_action: left_dbl,
                        right_mouse_up_action: right_up,
                        middle_mouse_up_action: middle_up,
                        mouse_over_action: over,
                        mouse_leave_action: leave,
                        mouse_scroll_up_action: scroll_up,
                        mouse_scroll_down_action: scroll_down,
                    });
                }
            }
        }

        hit_boxes
    }
}

fn resolve_meter_range(
    meter: &MeterConfig,
    state: &SkinState,
    current_val: f64,
    default_max: f64,
) -> (f64, f64) {
    let measure_cfg = meter
        .measure_name
        .as_deref()
        .and_then(|m| state.config.measures.get(&m.to_ascii_lowercase()));

    let min_val = meter
        .get("minvalue")
        .or_else(|| measure_cfg.and_then(|m| m.get("minvalue")))
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);

    let max_val = meter
        .get("maxvalue")
        .or_else(|| measure_cfg.and_then(|m| m.get("maxvalue")))
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or_else(|| {
            if let Some(m) = measure_cfg {
                let m_type = m.measure_type.to_ascii_lowercase();
                if m_type == "cpu"
                    || m_type == "memory"
                    || m_type == "physicalmemory"
                    || m_type == "swapmemory"
                {
                    return 100.0;
                }
                if m_type == "nowplaying"
                    || m.plugin
                        .as_deref()
                        .unwrap_or("")
                        .to_ascii_lowercase()
                        .contains("nowplaying")
                {
                    let p_type = m.get("playertype").unwrap_or("").to_ascii_lowercase();
                    if p_type == "progress" || p_type == "volume" {
                        return 100.0;
                    }
                }
            }
            if current_val > 1.0 && current_val <= 100.0 {
                100.0
            } else {
                default_max
            }
        });

    (min_val, max_val)
}

fn parse_point_nums(
    s: &str,
    vars: &VariableMap,
    current_section: Option<&str>,
    measures: Option<&HashMap<String, MeasureValue>>,
) -> Vec<f64> {
    let (_, nums) = tokenize_shape_declaration(&format!("dummy {}", s), vars, current_section, measures);
    nums
}


