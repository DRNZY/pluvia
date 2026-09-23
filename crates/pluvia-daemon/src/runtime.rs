use crate::display::gnome_bridge::GnomeBridgeSurface;
use crate::display::layer_shell::LayerShellSurface;
use crate::display::mock::MockDesktopSurface;
use crate::display::x11::X11Surface;
use crate::display::{Anchor, BackendType, DesktopSurface, DisplayError, SurfaceBounds};
use pluvia_core::ini::{parse_skin_file, ParseError, SkinConfig};
use pluvia_core::measures::{create_measure, Measure, MeasureValue};
use pluvia_core::render::meter_renderer::{MeterRenderer, SkinState};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use thiserror::Error;
use tokio::sync::RwLock;

#[derive(Error, Debug)]
pub enum RuntimeError {
    #[error("Skin not found: {0}")]
    SkinNotFound(String),
    #[error("Skin already loaded: {0}")]
    SkinAlreadyLoaded(String),
    #[error("Failed to parse skin: {0}")]
    Parse(#[from] ParseError),
    #[error("Display error: {0}")]
    Display(#[from] DisplayError),
    #[error("Render error: {0}")]
    Render(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonState {
    pub status: String,
    pub uptime_secs: u64,
    pub active_skins: Vec<String>,
    pub backend: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkinInfo {
    pub id: String,
    pub path: String,
    pub update_rate_ms: u64,
    pub measures_count: usize,
    pub meters_count: usize,
    pub bounds: SurfaceBounds,
}

pub struct SkinInstance {
    pub id: String,
    pub path: PathBuf,
    pub config: SkinConfig,
    pub measures: HashMap<String, Box<dyn Measure>>,
    pub measure_values: HashMap<String, MeasureValue>,
    pub surface: Box<dyn DesktopSurface>,
    pub tick_count: u64,
    pub last_tick: Instant,
}

pub struct SkinRuntime {
    backend: BackendType,
    skins: HashMap<String, SkinInstance>,
    renderer: MeterRenderer,
    start_time: Instant,
}

impl Default for SkinRuntime {
    fn default() -> Self {
        Self::new(BackendType::detect())
    }
}

impl SkinRuntime {
    pub fn new(backend: BackendType) -> Self {
        Self {
            backend,
            skins: HashMap::new(),
            renderer: MeterRenderer::new(),
            start_time: Instant::now(),
        }
    }

    pub fn backend(&self) -> BackendType {
        self.backend
    }

    fn calculate_bounds(&self, config: &SkinConfig) -> SurfaceBounds {
        let mut max_w: f64 = 0.0;
        let mut max_h: f64 = 0.0;

        for meter in config.meters.values() {
            let x = meter
                .properties
                .get("x")
                .map(|s| config.variables.expand(s))
                .and_then(|s| {
                    pluvia_core::formulas::eval_formula(&s, &config.variables)
                        .ok()
                        .or_else(|| s.trim().parse::<f64>().ok())
                })
                .unwrap_or(0.0);

            let y = meter
                .properties
                .get("y")
                .map(|s| config.variables.expand(s))
                .and_then(|s| {
                    pluvia_core::formulas::eval_formula(&s, &config.variables)
                        .ok()
                        .or_else(|| s.trim().parse::<f64>().ok())
                })
                .unwrap_or(0.0);

            let w = meter.w.unwrap_or(0.0);
            let h = meter.h.unwrap_or(0.0);

            let mut shape_w: f64 = 0.0;
            let mut shape_h: f64 = 0.0;
            if meter.meter_type.eq_ignore_ascii_case("shape") {
                for (k, v) in &meter.properties {
                    if k.starts_with("shape") {
                        let parts: Vec<&str> = v.split('|').collect();
                        if let Some(first) = parts.first() {
                            let exp = config.variables.expand(first);
                            let nums: Vec<f64> = exp
                                .split(&[' ', ','][..])
                                .map(str::trim)
                                .filter(|t| !t.is_empty())
                                .filter_map(|t| {
                                    pluvia_core::formulas::eval_formula(t, &config.variables)
                                        .ok()
                                        .or_else(|| t.parse::<f64>().ok())
                                })
                                .collect();
                            if exp.to_ascii_lowercase().contains("rectangle") && nums.len() >= 4 {
                                shape_w = shape_w.max(nums[0] + nums[2]);
                                shape_h = shape_h.max(nums[1] + nums[3]);
                            } else if exp.to_ascii_lowercase().contains("ellipse") && nums.len() >= 3 {
                                let rx = nums[2];
                                let ry = if nums.len() >= 4 { nums[3] } else { rx };
                                shape_w = shape_w.max(nums[0] + rx);
                                shape_h = shape_h.max(nums[1] + ry);
                            }
                        }
                    }
                }
            }

            let eff_w = if w > 0.0 { w } else { shape_w.max(100.0) };
            let eff_h = if h > 0.0 { h } else { shape_h.max(40.0) };

            let align = meter
                .get("stringalign")
                .map(pluvia_core::render::TextAlign::parse)
                .unwrap_or(pluvia_core::render::TextAlign::Left);

            let right = match align {
                pluvia_core::render::TextAlign::Center => (x + eff_w / 2.0).max(eff_w).max(x),
                pluvia_core::render::TextAlign::Right => x.max(eff_w),
                _ => x + eff_w,
            };

            if right > max_w {
                max_w = right;
            }
            if y + eff_h > max_h {
                max_h = y + eff_h;
            }
        }

        // Check widget/skin size variables
        for var_key in &["widgetwidth", "skinwidth", "width"] {
            if let Some(val_str) = config.variables.get(var_key) {
                let exp = config.variables.expand(val_str);
                if let Ok(v) = pluvia_core::formulas::eval_formula(&exp, &config.variables) {
                    max_w = max_w.max(v);
                } else if let Ok(v) = exp.trim().parse::<f64>() {
                    max_w = max_w.max(v);
                }
            }
        }
        for var_key in &["widgetheight", "skinheight", "height"] {
            if let Some(val_str) = config.variables.get(var_key) {
                let exp = config.variables.expand(val_str);
                if let Ok(v) = pluvia_core::formulas::eval_formula(&exp, &config.variables) {
                    max_h = max_h.max(v);
                } else if let Ok(v) = exp.trim().parse::<f64>() {
                    max_h = max_h.max(v);
                }
            }
        }

        let pad = config
            .variables
            .get("widgetpadding")
            .or_else(|| config.variables.get("paddingbase"))
            .and_then(|s| {
                pluvia_core::formulas::eval_formula(s, &config.variables)
                    .ok()
                    .or_else(|| s.trim().parse::<f64>().ok())
            })
            .unwrap_or(0.0);
        if pad > 0.0 {
            max_w += pad * 2.0;
            max_h += pad * 2.0;
        }

        let rainmeter_sec = config.raw_sections.get("rainmeter");
        if let Some(sec) = rainmeter_sec {
            if let Some(sw) = sec
                .get("skinwidth")
                .or_else(|| sec.get("windoww"))
                .and_then(|v| v.parse::<f64>().ok())
            {
                max_w = max_w.max(sw);
            }
            if let Some(sh) = sec
                .get("skinheight")
                .or_else(|| sec.get("windowh"))
                .and_then(|v| v.parse::<f64>().ok())
            {
                max_h = max_h.max(sh);
            }
        }

        let w = (max_w.ceil() as u32).max(200);
        let h = (max_h.ceil() as u32).max(180);

        let win_x = rainmeter_sec
            .and_then(|s| s.get("windowx").or_else(|| s.get("skinx")))
            .and_then(|v| v.parse::<i32>().ok())
            .or_else(|| config.variables.get("skinx").and_then(|v| v.parse::<i32>().ok()))
            .unwrap_or(0);
        let win_y = rainmeter_sec
            .and_then(|s| s.get("windowy").or_else(|| s.get("skiny")))
            .and_then(|v| v.parse::<i32>().ok())
            .or_else(|| config.variables.get("skiny").and_then(|v| v.parse::<i32>().ok()))
            .unwrap_or(0);

        SurfaceBounds::new(win_x, win_y, w, h)
    }

    fn create_surface(&self, id: &str, bounds: SurfaceBounds) -> Box<dyn DesktopSurface> {
        match self.backend {
            BackendType::Mock => Box::new(MockDesktopSurface::new(id, bounds)),
            BackendType::X11 => Box::new(X11Surface::new(id, bounds, 0)),
            BackendType::WlrLayerShell => Box::new(LayerShellSurface::new(
                id,
                bounds,
                crate::display::layer_shell::Layer::Background,
                "",
                Anchor::TopLeft,
                (0, 0),
            )),
            BackendType::GnomeWayland => Box::new(GnomeBridgeSurface::new_xwayland_fallback(id, bounds, 0)),
        }
    }

    pub fn load_skin<P: AsRef<Path>>(&mut self, path: P) -> Result<SkinInfo, RuntimeError> {
        let path_ref = path.as_ref();
        let path_buf = if path_ref.is_relative() {
            std::env::current_dir()?.join(path_ref)
        } else {
            path_ref.to_path_buf()
        };

        let config = parse_skin_file(&path_buf)?;

        // Derive skin ID from relative path components to prevent collisions between suites
        let stem = path_buf
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Skin");
        let parent = path_buf
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("");
        let grand = path_buf
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("");

        let base_id = if parent.is_empty() || parent.eq_ignore_ascii_case("skins") {
            stem.to_string()
        } else if stem.eq_ignore_ascii_case(parent) {
            parent.to_string()
        } else {
            format!("{}/{}", parent, stem)
        };

        let mut id = base_id;
        if self.skins.contains_key(&id) && !grand.is_empty() && !grand.eq_ignore_ascii_case("skins") {
            id = format!("{}/{}", grand, id);
        }

        if self.skins.contains_key(&id) {
            return Err(RuntimeError::SkinAlreadyLoaded(id));
        }

        // Register bundled fonts from @Resources/Fonts
        Self::register_skin_fonts(&config.skin_dir);
        if let Some(parent) = config.skin_dir.parent() {
            Self::register_skin_fonts(parent);
        }

        let mut measures: HashMap<String, Box<dyn Measure>> = HashMap::new();
        let mut measure_values: HashMap<String, MeasureValue> = HashMap::new();

        for name in &config.measure_order {
            if let Some(measure_cfg) = config.measures.get(name) {
                if let Some(mut m) = create_measure(measure_cfg) {
                    let val = m.update_with_context(&config.variables, &measure_values);
                    measures.insert(name.clone(), m);
                    measure_values.insert(name.clone(), val);
                }
            }
        }

        let bounds = self.calculate_bounds(&config);
        let surface = self.create_surface(&id, bounds);

        let mut instance = SkinInstance {
            id: id.clone(),
            path: path_buf.clone(),
            config,
            measures,
            measure_values,
            surface,
            tick_count: 0,
            last_tick: Instant::now(),
        };

        // Apply initial measure actions (e.g. OnUpdateAction, IfMatchAction) in definition order
        let initial_order = instance.config.measure_order.clone();
        for name in &initial_order {
            if let Some(val) = instance.measure_values.get(name).cloned() {
                Self::apply_measure_actions(&mut instance, name, &val, None);
            }
        }

        // Render initial frame
        Self::render_instance(&self.renderer, &mut instance)?;

        let info = SkinInfo {
            id: id.clone(),
            path: path_buf.to_string_lossy().to_string(),
            update_rate_ms: instance.config.update_rate_ms,
            measures_count: instance.measures.len(),
            meters_count: instance.config.meters.len(),
            bounds: instance.surface.bounds(),
        };

        self.skins.insert(id, instance);
        Ok(info)
    }

    pub fn unload_skin(&mut self, id: &str) -> Result<(), RuntimeError> {
        let key = Self::find_skin_key(&self.skins, id)
            .ok_or_else(|| RuntimeError::SkinNotFound(id.to_string()))?;
        if let Some(mut skin) = self.skins.remove(&key) {
            skin.surface.destroy()?;
            Ok(())
        } else {
            Err(RuntimeError::SkinNotFound(id.to_string()))
        }
    }

    pub fn refresh_skin(&mut self, id: &str) -> Result<(), RuntimeError> {
        let key = Self::find_skin_key(&self.skins, id)
            .ok_or_else(|| RuntimeError::SkinNotFound(id.to_string()))?;
        let skin = self.skins.get_mut(&key).unwrap();

        let new_config = parse_skin_file(&skin.path)?;
        skin.config = new_config;

        Self::register_skin_fonts(&skin.config.skin_dir);
        if let Some(parent) = skin.config.skin_dir.parent() {
            Self::register_skin_fonts(parent);
        }

        // Recreate measures in definition order
        skin.measures.clear();
        skin.measure_values.clear();
        for name in &skin.config.measure_order {
            if let Some(measure_cfg) = skin.config.measures.get(name) {
                if let Some(mut m) = create_measure(measure_cfg) {
                    let val = m.update_with_context(&skin.config.variables, &skin.measure_values);
                    skin.measures.insert(name.clone(), m);
                    skin.measure_values.insert(name.clone(), val);
                }
            }
        }

        skin.tick_count = 0;
        skin.last_tick = Instant::now();
        Self::render_instance(&self.renderer, skin)?;
        Ok(())
    }

    pub fn refresh_all(&mut self) -> Result<usize, RuntimeError> {
        let ids: Vec<String> = self.skins.keys().cloned().collect();
        let mut count = 0;
        for id in ids {
            if self.refresh_skin(&id).is_ok() {
                count += 1;
            }
        }
        Ok(count)
    }

    pub fn find_skin_key(skins: &HashMap<String, SkinInstance>, query: &str) -> Option<String> {
        let q = query.trim();
        if skins.contains_key(q) {
            return Some(q.to_string());
        }
        for (key, instance) in skins {
            if key.eq_ignore_ascii_case(q) {
                return Some(key.clone());
            }
            let path_str = instance.path.to_string_lossy();
            if path_str == q || path_str.ends_with(q) {
                return Some(key.clone());
            }
            let q_norm = q.replace(" / ", "/").replace('\\', "/").to_ascii_lowercase();
            let path_norm = path_str.replace('\\', "/").to_ascii_lowercase();
            if path_norm.contains(&q_norm) || path_norm.ends_with(&q_norm) {
                return Some(key.clone());
            }
            if let Some(stem) = instance.path.file_stem().and_then(|s| s.to_str()) {
                if stem.eq_ignore_ascii_case(q) {
                    return Some(key.clone());
                }
            }
        }
        None
    }

    fn find_skin_mut<'a>(
        skins: &'a mut HashMap<String, SkinInstance>,
        id: &str,
    ) -> Option<&'a mut SkinInstance> {
        let key = Self::find_skin_key(skins, id)?;
        skins.get_mut(&key)
    }

    pub fn set_variable(
        &mut self,
        id: &str,
        key: &str,
        value: &str,
    ) -> Result<(), RuntimeError> {
        let skin = Self::find_skin_mut(&mut self.skins, id)
            .ok_or_else(|| RuntimeError::SkinNotFound(id.to_string()))?;

        skin.config.variables.set(key, value);
        if let Some(sec) = skin.config.raw_sections.get_mut("variables") {
            sec.insert(key.to_ascii_lowercase(), value.to_string());
        }

        skin.last_tick = Instant::now();
        Self::render_instance(&self.renderer, skin)?;
        Ok(())
    }

    pub fn set_position(
        &mut self,
        id: &str,
        x: i32,
        y: i32,
    ) -> Result<(), RuntimeError> {
        let skin = Self::find_skin_mut(&mut self.skins, id)
            .ok_or_else(|| RuntimeError::SkinNotFound(id.to_string()))?;

        let mut bounds = skin.surface.bounds();
        bounds.x = x;
        bounds.y = y;
        skin.surface.set_bounds(bounds)?;
        Ok(())
    }

    pub fn set_opacity(
        &mut self,
        id: &str,
        opacity: f64,
    ) -> Result<(), RuntimeError> {
        let skin = Self::find_skin_mut(&mut self.skins, id)
            .ok_or_else(|| RuntimeError::SkinNotFound(id.to_string()))?;

        skin.surface.set_opacity(opacity)?;
        Ok(())
    }

    pub fn list_skins(&self) -> Vec<SkinInfo> {
        self.skins
            .values()
            .map(|skin| SkinInfo {
                id: skin.id.clone(),
                path: skin.path.to_string_lossy().to_string(),
                update_rate_ms: skin.config.update_rate_ms,
                measures_count: skin.measures.len(),
                meters_count: skin.config.meters.len(),
                bounds: skin.surface.bounds(),
            })
            .collect()
    }

    pub fn get_state(&self) -> DaemonState {
        DaemonState {
            status: "running".to_string(),
            uptime_secs: self.start_time.elapsed().as_secs(),
            active_skins: self.skins.keys().cloned().collect(),
            backend: format!("{:?}", self.backend),
        }
    }

    pub fn get_skin(&self, id: &str) -> Option<&SkinInstance> {
        let key = Self::find_skin_key(&self.skins, id)?;
        self.skins.get(&key)
    }

    pub fn get_skin_mut(&mut self, id: &str) -> Option<&mut SkinInstance> {
        Self::find_skin_mut(&mut self.skins, id)
    }

    fn execute_tick(renderer: &MeterRenderer, skin: &mut SkinInstance) -> Result<(), RuntimeError> {
        skin.last_tick = Instant::now();
        skin.tick_count = skin.tick_count.wrapping_add(1);

        let measure_names: Vec<String> = skin.config.measure_order.clone();
        for name in measure_names {
            let divider = skin
                .config
                .measures
                .get(&name)
                .map(|m| m.update_divider)
                .unwrap_or(1)
                .max(1);

            if skin.tick_count % (divider as u64) == 0 {
                let old_val = skin.measure_values.get(&name).cloned();
                let new_val = if let Some(m) = skin.measures.get_mut(&name) {
                    m.update_with_context(&skin.config.variables, &skin.measure_values)
                } else {
                    continue;
                };
                skin.measure_values.insert(name.clone(), new_val.clone());
                Self::apply_measure_actions(skin, &name, &new_val, old_val.as_ref());
            }
        }

        Self::render_instance(renderer, skin)?;
        Ok(())
    }

    fn apply_measure_actions(
        skin: &mut SkinInstance,
        name: &str,
        new_val: &MeasureValue,
        old_val: Option<&MeasureValue>,
    ) {
        let props = match skin.config.measures.get(name) {
            Some(m) => m.properties.clone(),
            None => return,
        };

        // 1. OnUpdateAction
        if let Some(action) = props.get("onupdateaction") {
            let expanded = skin.config.variables.expand_with_context(
                action,
                Some(name),
                Some(&skin.measure_values),
            );
            Self::execute_bangs(skin, &expanded);
        }

        // 2. OnChangeAction
        if let Some(action) = props.get("onchangeaction") {
            if old_val.map(|o| o.to_string_val()) != Some(new_val.to_string_val()) {
                let expanded = skin.config.variables.expand_with_context(
                    action,
                    Some(name),
                    Some(&skin.measure_values),
                );
                Self::execute_bangs(skin, &expanded);
            }
        }

        // 3. IfMatch
        if let Some(pattern) = props.get("ifmatch") {
            let exp_pattern = skin.config.variables.expand_with_context(
                pattern,
                Some(name),
                Some(&skin.measure_values),
            );
            let val_str = new_val.to_string_val();
            let is_match = val_str == exp_pattern
                || regex::Regex::new(&exp_pattern)
                    .map(|r| r.is_match(&val_str))
                    .unwrap_or(false);

            if is_match {
                if let Some(match_act) = props.get("ifmatchaction") {
                    let expanded = skin.config.variables.expand_with_context(
                        match_act,
                        Some(name),
                        Some(&skin.measure_values),
                    );
                    Self::execute_bangs(skin, &expanded);
                }
            } else if let Some(not_match_act) = props.get("ifnotmatchaction") {
                let expanded = skin.config.variables.expand_with_context(
                    not_match_act,
                    Some(name),
                    Some(&skin.measure_values),
                );
                Self::execute_bangs(skin, &expanded);
            }
        }
    }

    fn execute_bangs(skin: &mut SkinInstance, bang_str: &str) {
        let bangs = pluvia_core::bangs::parse_bangs(bang_str);
        for bang in bangs {
            match bang {
                pluvia_core::bangs::Bang::SetVariable { name, value, .. } => {
                    let exp = skin.config.variables.expand_with_context(
                        &value,
                        None,
                        Some(&skin.measure_values),
                    );
                    let final_val = if let Ok(n) =
                        pluvia_core::formulas::eval_formula(&exp, &skin.config.variables)
                    {
                        format!("{}", n)
                    } else {
                        exp
                    };
                    skin.config.variables.set(&name, &final_val);
                    if let Some(sec) = skin.config.raw_sections.get_mut("variables") {
                        sec.insert(name.to_ascii_lowercase(), final_val);
                    }
                }
                pluvia_core::bangs::Bang::SetOption {
                    section,
                    key,
                    value,
                    ..
                } => {
                    let exp = skin.config.variables.expand_with_context(
                        &value,
                        None,
                        Some(&skin.measure_values),
                    );
                    let sec_lower = section.to_ascii_lowercase();
                    let key_lower = key.to_ascii_lowercase();
                    if let Some(meter) = skin.config.meters.get_mut(&sec_lower) {
                        meter.properties.insert(key_lower.clone(), exp.clone());
                        if key_lower == "text" {
                            meter.text = Some(exp.clone());
                        } else if key_lower == "x" {
                            meter.x = Some(exp.clone());
                        } else if key_lower == "y" {
                            meter.y = Some(exp.clone());
                        } else if key_lower == "w" {
                            meter.w = exp.parse::<f64>().ok();
                        } else if key_lower == "h" {
                            meter.h = exp.parse::<f64>().ok();
                        } else if key_lower == "fontcolor" {
                            meter.font_color = Some(exp.clone());
                        }
                    }
                }
                pluvia_core::bangs::Bang::CommandMeasure { measure, command, .. } => {
                    let m_lower = measure.to_ascii_lowercase();
                    if let Some(m) = skin.measures.get_mut(&m_lower) {
                        m.command(&command);
                    }
                }
                pluvia_core::bangs::Bang::UpdateMeasure { name, .. } => {
                    let m_lower = name.to_ascii_lowercase();
                    if m_lower == "*" {
                        for (k, m) in skin.measures.iter_mut() {
                            let val = m.update_with_context(&skin.config.variables, &skin.measure_values);
                            skin.measure_values.insert(k.clone(), val);
                        }
                    } else if let Some(m) = skin.measures.get_mut(&m_lower) {
                        let val = m.update_with_context(&skin.config.variables, &skin.measure_values);
                        skin.measure_values.insert(m_lower, val);
                    }
                }
                pluvia_core::bangs::Bang::ShowMeter { name, .. } => {
                    let m_lower = name.to_ascii_lowercase();
                    if let Some(meter) = skin.config.meters.get_mut(&m_lower) {
                        meter.hidden = false;
                    }
                }
                pluvia_core::bangs::Bang::HideMeter { name, .. } => {
                    let m_lower = name.to_ascii_lowercase();
                    if let Some(meter) = skin.config.meters.get_mut(&m_lower) {
                        meter.hidden = true;
                    }
                }
                pluvia_core::bangs::Bang::ToggleMeter { name, .. } => {
                    let m_lower = name.to_ascii_lowercase();
                    if let Some(meter) = skin.config.meters.get_mut(&m_lower) {
                        meter.hidden = !meter.hidden;
                    }
                }
                pluvia_core::bangs::Bang::WriteKeyValue { section, key, value, file } => {
                    let target_path = file.unwrap_or_else(|| skin.path.clone());
                    let exp = skin.config.variables.expand_with_context(&value, None, Some(&skin.measure_values));
                    let _ = pluvia_core::bangs::write_key_value_to_file(target_path, &section, &key, &exp);
                }
                pluvia_core::bangs::Bang::Execute(cmd) => {
                    let _ = pluvia_core::bangs::execute_command(&cmd);
                }
                _ => {}
            }
        }
    }

    /// Handles mouse click at (x, y) relative to skin canvas, executing any matched meter bangs.
    pub fn handle_mouse_click(
        &mut self,
        id: &str,
        x: f64,
        y: f64,
        button: u32,
    ) -> Result<bool, RuntimeError> {
        let key = match Self::find_skin_key(&self.skins, id) {
            Some(k) => k,
            None => return Ok(false),
        };

        let hit_boxes = {
            let skin = self.skins.get(&key).unwrap();
            let state = SkinState::new(skin.config.clone(), skin.measure_values.clone());
            self.renderer.get_interactive_hit_boxes(&state)
        };

        let mut action_to_execute = None;
        for hb in hit_boxes.iter().rev() {
            if hb.contains(x, y) {
                let action = match button {
                    1 => hb.left_mouse_up_action.as_ref().or(hb.left_mouse_down_action.as_ref()),
                    2 => hb.middle_mouse_up_action.as_ref(),
                    3 => hb.right_mouse_up_action.as_ref(),
                    _ => None,
                };
                if let Some(act) = action {
                    action_to_execute = Some((hb.meter_name.clone(), act.clone()));
                    break;
                }
            }
        }

        if let Some((meter_name, act_str)) = action_to_execute {
            if let Some(skin) = self.skins.get_mut(&key) {
                let expanded = skin.config.variables.expand_with_context(
                    &act_str,
                    Some(&meter_name),
                    Some(&skin.measure_values),
                );
                Self::execute_bangs(skin, &expanded);
                let _ = Self::render_instance(&self.renderer, skin);
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Feeds live audio PCM buffer into all active audio level measures across loaded skins.
    pub fn feed_audio_samples(&mut self, samples: &[f32]) {
        for skin in self.skins.values_mut() {
            for measure in skin.measures.values_mut() {
                measure.feed_audio(samples);
            }
        }
    }

    pub fn force_tick_skin(&mut self, id: &str) -> Result<(), RuntimeError> {
        let skin = Self::find_skin_mut(&mut self.skins, id)
            .ok_or_else(|| RuntimeError::SkinNotFound(id.to_string()))?;

        Self::execute_tick(&self.renderer, skin)
    }

    pub fn tick_skin(&mut self, id: &str) -> Result<(), RuntimeError> {
        let skin = Self::find_skin_mut(&mut self.skins, id)
            .ok_or_else(|| RuntimeError::SkinNotFound(id.to_string()))?;

        if skin.config.update_rate_ms == 0 {
            return Ok(());
        }

        if skin.last_tick.elapsed().as_millis() < skin.config.update_rate_ms as u128 {
            return Ok(());
        }

        Self::execute_tick(&self.renderer, skin)
    }

    pub fn tick_all(&mut self) -> Result<(), RuntimeError> {
        for skin in self.skins.values_mut() {
            skin.surface.poll_events();
        }

        let ids: Vec<String> = self.skins.keys().cloned().collect();
        for id in ids {
            let _ = self.tick_skin(&id);
        }
        Ok(())
    }

    fn render_instance(
        renderer: &MeterRenderer,
        instance: &mut SkinInstance,
    ) -> Result<(), RuntimeError> {
        let state = SkinState::new(instance.config.clone(), instance.measure_values.clone());
        let bounds = instance.surface.bounds();
        let w = bounds.width.max(1) as i32;
        let h = bounds.height.max(1) as i32;

        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, w, h)
            .map_err(|e| RuntimeError::Render(format!("Cairo surface creation error: {e}")))?;

        let hit_mask = renderer
            .render_to_surface(&state, &surface)
            .map_err(|e| RuntimeError::Render(format!("Render error: {e}")))?;

        instance.surface.update_surface(&surface, &hit_mask)?;
        // Clears frame damage per tick to prevent unbounded memory growth
        instance.surface.clear_damage();

        Ok(())
    }

    fn register_skin_fonts(base_dir: &Path) {
        let vfs = pluvia_core::vfs::VfsResolver::new();
        let mut curr = Some(base_dir);
        while let Some(dir) = curr {
            if let Some(fonts_dir) = vfs.resolve(dir, "@Resources/Fonts") {
                if fonts_dir.is_dir() {
                    if let Ok(entries) = std::fs::read_dir(&fonts_dir) {
                        for entry in entries.flatten() {
                            let path = entry.path();
                            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                                let ext_lower = ext.to_ascii_lowercase();
                                if ext_lower == "otf"
                                    || ext_lower == "ttf"
                                    || ext_lower == "woff"
                                    || ext_lower == "woff2"
                                {
                                    pluvia_core::render::add_application_font(&path);
                                }
                            }
                        }
                    }
                    break;
                }
            }
            curr = dir.parent();
        }
    }
}

/// Spawns a background ticker loop managing skin tick intervals.
pub fn start_background_ticker(
    runtime: Arc<RwLock<SkinRuntime>>,
    mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(50));
        let mut env_check_counter = 0u32;
        let mut is_game_fullscreen = false;
        // (skin_id -> was_auto_hidden_by_window)
        let mut auto_hidden: std::collections::HashMap<String, bool> = std::collections::HashMap::new();

        loop {
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        break;
                    }
                }
                _ = interval.tick() => {
                    env_check_counter = env_check_counter.wrapping_add(1);

                    // Every ~500ms: check fullscreen state and auto-hide overlapping windows
                    if env_check_counter % 10 == 0 {
                        // Run blocking X11 queries off the async runtime thread
                        let (fullscreen, covering_rects) = tokio::task::spawn_blocking(|| {
                            let fs = crate::display::x11::is_fullscreen_window_active();
                            let rects = crate::display::x11::get_visible_normal_window_rects();
                            (fs, rects)
                        })
                        .await
                        .unwrap_or((false, Vec::new()));

                        is_game_fullscreen = fullscreen;

                        let mut rt = runtime.write().await;
                        let skin_ids: Vec<String> = rt.skins.keys().cloned().collect();

                        // Compute visibility changes (read pass)
                        let mut visibility_changes: Vec<(String, bool)> = Vec::new();
                        for id in &skin_ids {
                            if let Some(skin) = rt.skins.get(id) {
                                let sb = skin.surface.bounds();
                                let is_covered = covering_rects.iter().any(|wr| {
                                    wr.x <= sb.x
                                        && wr.y <= sb.y
                                        && wr.x + wr.width as i32 >= sb.x + sb.width as i32
                                        && wr.y + wr.height as i32 >= sb.y + sb.height as i32
                                });
                                let was_hidden = auto_hidden.get(id).copied().unwrap_or(false);
                                if is_covered && !was_hidden {
                                    visibility_changes.push((id.clone(), false)); // hide
                                } else if !is_covered && was_hidden {
                                    visibility_changes.push((id.clone(), true)); // show
                                }
                            }
                        }

                        // Apply visibility changes (write pass)
                        for (id, visible) in visibility_changes {
                            auto_hidden.insert(id.clone(), !visible);
                            if let Some(skin) = rt.skins.get_mut(&id) {
                                let _ = skin.surface.set_visible(visible);
                            }
                        }

                        // Cleanup entries for unloaded skins
                        auto_hidden.retain(|k, _| rt.skins.contains_key(k));
                    }

                    if is_game_fullscreen {
                        continue;
                    }

                    let mut rt = runtime.write().await;
                    let _ = rt.tick_all();
                }
            }
        }
    })
}
