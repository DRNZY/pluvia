use super::{BackendType, DesktopSurface, DisplayError, SurfaceBounds};
use pluvia_core::render::{AlphaHitMask, Rect};

/// X11 desktop window surface with EWMH desktop hints and XShape/XFixes click-through mask.
#[derive(Debug, Clone)]
pub struct X11Surface {
    id: String,
    bounds: SurfaceBounds,
    window_id: u32,
    window_type_atom: String,
    state_atoms: Vec<String>,
    desktop_atom_value: u32,
    visible: bool,
    destroyed: bool,
    shape_input_rects: Vec<Rect>,
    damage_rects: Vec<Rect>,
}

impl X11Surface {
    pub fn new(id: impl Into<String>, bounds: SurfaceBounds, window_id: u32) -> Self {
        Self {
            id: id.into(),
            bounds,
            window_id,
            window_type_atom: "_NET_WM_WINDOW_TYPE_DESKTOP".to_string(),
            state_atoms: vec![
                "_NET_WM_STATE_BELOW".to_string(),
                "_NET_WM_STATE_STICKY".to_string(),
            ],
            desktop_atom_value: 0xFFFFFFFF,
            visible: true,
            destroyed: false,
            shape_input_rects: Vec::new(),
            damage_rects: Vec::new(),
        }
    }

    pub fn window_id(&self) -> u32 {
        self.window_id
    }

    pub fn window_type_atom(&self) -> &str {
        &self.window_type_atom
    }

    pub fn has_state_atom(&self, atom: &str) -> bool {
        self.state_atoms.iter().any(|a| a == atom)
    }

    pub fn state_atoms(&self) -> &[String] {
        &self.state_atoms
    }

    pub fn desktop_atom_value(&self) -> u32 {
        self.desktop_atom_value
    }

    pub fn shape_input_rects(&self) -> &[Rect] {
        &self.shape_input_rects
    }

    pub fn damage_rects(&self) -> &[Rect] {
        &self.damage_rects
    }
}

impl DesktopSurface for X11Surface {
    fn id(&self) -> &str {
        &self.id
    }

    fn backend_type(&self) -> BackendType {
        BackendType::X11
    }

    fn bounds(&self) -> SurfaceBounds {
        self.bounds
    }

    fn set_bounds(&mut self, bounds: SurfaceBounds) -> Result<(), DisplayError> {
        if self.destroyed {
            return Err(DisplayError::SurfaceDestroyed);
        }
        self.bounds = bounds;
        Ok(())
    }

    fn is_visible(&self) -> bool {
        self.visible && !self.destroyed
    }

    fn set_visible(&mut self, visible: bool) -> Result<(), DisplayError> {
        if self.destroyed {
            return Err(DisplayError::SurfaceDestroyed);
        }
        self.visible = visible;
        Ok(())
    }

    fn update_surface(
        &mut self,
        surface: &cairo::ImageSurface,
        hit_mask: &AlphaHitMask,
    ) -> Result<(), DisplayError> {
        if self.destroyed {
            return Err(DisplayError::SurfaceDestroyed);
        }
        self.shape_input_rects = hit_mask.to_rectangles();
        self.bounds.width = surface.width() as u32;
        self.bounds.height = surface.height() as u32;
        self.damage_rects.push(Rect::new(
            0.0,
            0.0,
            surface.width() as f64,
            surface.height() as f64,
        ));
        Ok(())
    }

    fn update_hit_mask(&mut self, hit_mask: &AlphaHitMask) -> Result<(), DisplayError> {
        if self.destroyed {
            return Err(DisplayError::SurfaceDestroyed);
        }
        self.shape_input_rects = hit_mask.to_rectangles();
        Ok(())
    }

    fn destroy(&mut self) -> Result<(), DisplayError> {
        self.destroyed = true;
        self.shape_input_rects.clear();
        Ok(())
    }

    fn is_destroyed(&self) -> bool {
        self.destroyed
    }

    fn clear_damage(&mut self) {
        self.damage_rects.clear();
    }
}
