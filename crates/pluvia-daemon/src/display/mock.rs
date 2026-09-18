use super::{BackendType, DesktopSurface, DisplayError, SurfaceBounds};
use pluvia_core::render::{AlphaHitMask, Rect};

/// Hermetic mock desktop surface for headless testing and verification.
#[derive(Debug, Clone)]
pub struct MockDesktopSurface {
    id: String,
    bounds: SurfaceBounds,
    visible: bool,
    destroyed: bool,
    update_count: usize,
    last_surface_size: Option<(u32, u32)>,
    damage_history: Vec<Rect>,
    hit_mask_rects: Vec<Rect>,
}

impl MockDesktopSurface {
    pub fn new(id: impl Into<String>, bounds: SurfaceBounds) -> Self {
        Self {
            id: id.into(),
            bounds,
            visible: true,
            destroyed: false,
            update_count: 0,
            last_surface_size: None,
            damage_history: Vec::new(),
            hit_mask_rects: Vec::new(),
        }
    }

    pub fn update_count(&self) -> usize {
        self.update_count
    }

    pub fn last_surface_size(&self) -> Option<(u32, u32)> {
        self.last_surface_size
    }

    pub fn damage_history(&self) -> &[Rect] {
        &self.damage_history
    }

    pub fn hit_mask_rectangles(&self) -> &[Rect] {
        &self.hit_mask_rects
    }
}

impl DesktopSurface for MockDesktopSurface {
    fn id(&self) -> &str {
        &self.id
    }

    fn backend_type(&self) -> BackendType {
        BackendType::Mock
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
        let w = surface.width() as u32;
        let h = surface.height() as u32;
        self.last_surface_size = Some((w, h));
        self.bounds.width = w;
        self.bounds.height = h;
        self.hit_mask_rects = hit_mask.to_rectangles();
        self.damage_history
            .push(Rect::new(0.0, 0.0, w as f64, h as f64));
        self.update_count += 1;
        Ok(())
    }

    fn update_hit_mask(&mut self, hit_mask: &AlphaHitMask) -> Result<(), DisplayError> {
        if self.destroyed {
            return Err(DisplayError::SurfaceDestroyed);
        }
        self.hit_mask_rects = hit_mask.to_rectangles();
        Ok(())
    }

    fn destroy(&mut self) -> Result<(), DisplayError> {
        self.destroyed = true;
        self.hit_mask_rects.clear();
        Ok(())
    }

    fn is_destroyed(&self) -> bool {
        self.destroyed
    }

    fn clear_damage(&mut self) {
        self.damage_history.clear();
    }
}
