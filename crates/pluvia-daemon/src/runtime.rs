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
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);
            let y = meter
                .properties
                .get("y")
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);
            let w = meter.w.unwrap_or(100.0);
            let h = meter.h.unwrap_or(40.0);
            if x + w > max_w {
                max_w = x + w;
            }
            if y + h > max_h {
                max_h = y + h;
            }
        }
        let w = (max_w.ceil() as u32).max(200);
        let h = (max_h.ceil() as u32).max(100);
        SurfaceBounds::new(0, 0, w, h)
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
            BackendType::GnomeWayland => Box::new(GnomeBridgeSurface::new_extension(id, bounds)),
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

        // Derive skin ID from parent folder name or file stem
        let id = path_buf
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .filter(|s| !s.is_empty())
            .or_else(|| {
                path_buf
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .filter(|s| !s.is_empty())
            })
            .unwrap_or("Skin")
            .to_string();

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

        for measure_cfg in config.measures.values() {
            if let Some(mut m) = create_measure(measure_cfg) {
                let val = m.update();
                measures.insert(measure_cfg.name.to_ascii_lowercase(), m);
                measure_values.insert(measure_cfg.name.to_ascii_lowercase(), val);
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
        if let Some(mut skin) = self.skins.remove(id) {
            skin.surface.destroy()?;
            Ok(())
        } else {
            Err(RuntimeError::SkinNotFound(id.to_string()))
        }
    }

    pub fn refresh_skin(&mut self, id: &str) -> Result<(), RuntimeError> {
        let skin = self
            .skins
            .get_mut(id)
            .ok_or_else(|| RuntimeError::SkinNotFound(id.to_string()))?;

        let new_config = parse_skin_file(&skin.path)?;
        skin.config = new_config;

        Self::register_skin_fonts(&skin.config.skin_dir);
        if let Some(parent) = skin.config.skin_dir.parent() {
            Self::register_skin_fonts(parent);
        }

        // Recreate measures
        skin.measures.clear();
        skin.measure_values.clear();
        for measure_cfg in skin.config.measures.values() {
            if let Some(mut m) = create_measure(measure_cfg) {
                let val = m.update();
                skin.measures
                    .insert(measure_cfg.name.to_ascii_lowercase(), m);
                skin.measure_values
                    .insert(measure_cfg.name.to_ascii_lowercase(), val);
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

    pub fn set_variable(
        &mut self,
        id: &str,
        key: &str,
        value: &str,
    ) -> Result<(), RuntimeError> {
        let skin = self
            .skins
            .get_mut(id)
            .ok_or_else(|| RuntimeError::SkinNotFound(id.to_string()))?;

        skin.config.variables.set(key, value);
        if let Some(sec) = skin.config.raw_sections.get_mut("variables") {
            sec.insert(key.to_ascii_lowercase(), value.to_string());
        }

        skin.last_tick = Instant::now();
        Self::render_instance(&self.renderer, skin)?;
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
        self.skins.get(id)
    }

    pub fn get_skin_mut(&mut self, id: &str) -> Option<&mut SkinInstance> {
        self.skins.get_mut(id)
    }

    fn execute_tick(renderer: &MeterRenderer, skin: &mut SkinInstance) -> Result<(), RuntimeError> {
        skin.last_tick = Instant::now();
        skin.tick_count = skin.tick_count.wrapping_add(1);

        // Update measures based on update_divider
        for (name, measure) in &mut skin.measures {
            let divider = skin
                .config
                .measures
                .get(name)
                .map(|m| m.update_divider)
                .unwrap_or(1)
                .max(1);

            if skin.tick_count % (divider as u64) == 0 {
                let val = measure.update();
                skin.measure_values.insert(name.clone(), val);
            }
        }

        Self::render_instance(renderer, skin)?;
        Ok(())
    }

    pub fn force_tick_skin(&mut self, id: &str) -> Result<(), RuntimeError> {
        let skin = self
            .skins
            .get_mut(id)
            .ok_or_else(|| RuntimeError::SkinNotFound(id.to_string()))?;

        Self::execute_tick(&self.renderer, skin)
    }

    pub fn tick_skin(&mut self, id: &str) -> Result<(), RuntimeError> {
        let skin = self
            .skins
            .get_mut(id)
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
        if let Some(fonts_dir) = vfs.resolve(base_dir, "@Resources/Fonts") {
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
            }
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
        loop {
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        break;
                    }
                }
                _ = interval.tick() => {
                    let mut rt = runtime.write().await;
                    let _ = rt.tick_all();
                }
            }
        }
    })
}
