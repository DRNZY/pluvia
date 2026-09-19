use super::x11::X11Surface;
use super::{BackendType, DesktopSurface, DisplayError, SurfaceBounds};
use pluvia_core::render::{AlphaHitMask, Rect};
use serde::{Deserialize, Serialize};

/// Operating mode for GNOME Wayland desktop integration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GnomeBridgeMode {
    /// Dedicated D-Bus bridge to `pluvia-shell@pluvia.org` GNOME Shell extension.
    Extension,
    /// Fallback desktop surface via XWayland desktop-layer window hints.
    XWaylandFallback,
}

/// Desktop surface bridge for GNOME Shell on Wayland.
#[derive(Debug, Clone)]
pub struct GnomeBridgeSurface {
    id: String,
    bounds: SurfaceBounds,
    mode: GnomeBridgeMode,
    extension_uuid: String,
    dbus_interface: String,
    fallback_x11: Option<X11Surface>,
    visible: bool,
    destroyed: bool,
    active_rects: Vec<Rect>,
}

impl GnomeBridgeSurface {
    /// Creates a surface using the native `pluvia-shell@pluvia.org` GNOME Shell extension D-Bus bridge.
    pub fn new_extension(id: impl Into<String>, bounds: SurfaceBounds) -> Self {
        Self {
            id: id.into(),
            bounds,
            mode: GnomeBridgeMode::Extension,
            extension_uuid: "pluvia-shell@pluvia.org".to_string(),
            dbus_interface: "org.pluvia.Shell".to_string(),
            fallback_x11: None,
            visible: true,
            destroyed: false,
            active_rects: Vec::new(),
        }
    }

    /// Creates a surface in fallback mode backed by an XWayland desktop window.
    pub fn new_xwayland_fallback(
        id: impl Into<String>,
        bounds: SurfaceBounds,
        fallback_window_id: u32,
    ) -> Self {
        let id_str = id.into();
        let fallback = X11Surface::new(&id_str, bounds, fallback_window_id);
        Self {
            id: id_str,
            bounds,
            mode: GnomeBridgeMode::XWaylandFallback,
            extension_uuid: "pluvia-shell@pluvia.org".to_string(),
            dbus_interface: "org.pluvia.Shell".to_string(),
            fallback_x11: Some(fallback),
            visible: true,
            destroyed: false,
            active_rects: Vec::new(),
        }
    }

    pub fn mode(&self) -> GnomeBridgeMode {
        self.mode
    }

    pub fn extension_uuid(&self) -> &str {
        &self.extension_uuid
    }

    pub fn dbus_interface(&self) -> &str {
        &self.dbus_interface
    }

    pub fn fallback_x11(&self) -> Option<&X11Surface> {
        self.fallback_x11.as_ref()
    }

    pub fn fallback_x11_mut(&mut self) -> Option<&mut X11Surface> {
        self.fallback_x11.as_mut()
    }

    pub fn active_rects(&self) -> &[Rect] {
        &self.active_rects
    }
}

impl DesktopSurface for GnomeBridgeSurface {
    fn id(&self) -> &str {
        &self.id
    }

    fn backend_type(&self) -> BackendType {
        BackendType::GnomeWayland
    }

    fn bounds(&self) -> SurfaceBounds {
        if let Some(x11) = &self.fallback_x11 {
            x11.bounds()
        } else {
            self.bounds
        }
    }

    fn set_bounds(&mut self, bounds: SurfaceBounds) -> Result<(), DisplayError> {
        if self.destroyed {
            return Err(DisplayError::SurfaceDestroyed);
        }
        self.bounds = bounds;
        if let Some(x11) = &mut self.fallback_x11 {
            x11.set_bounds(bounds)?;
        }
        Ok(())
    }

    fn is_visible(&self) -> bool {
        if let Some(x11) = &self.fallback_x11 {
            x11.is_visible()
        } else {
            self.visible && !self.destroyed
        }
    }

    fn set_visible(&mut self, visible: bool) -> Result<(), DisplayError> {
        if self.destroyed {
            return Err(DisplayError::SurfaceDestroyed);
        }
        self.visible = visible;
        if let Some(x11) = &mut self.fallback_x11 {
            x11.set_visible(visible)?;
        }
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
        self.active_rects = hit_mask.to_rectangles();
        self.bounds.width = surface.width() as u32;
        self.bounds.height = surface.height() as u32;

        if let Some(x11) = &mut self.fallback_x11 {
            x11.update_surface(surface, hit_mask)?;
        }
        Ok(())
    }

    fn update_hit_mask(&mut self, hit_mask: &AlphaHitMask) -> Result<(), DisplayError> {
        if self.destroyed {
            return Err(DisplayError::SurfaceDestroyed);
        }
        self.active_rects = hit_mask.to_rectangles();
        if let Some(x11) = &mut self.fallback_x11 {
            x11.update_hit_mask(hit_mask)?;
        }
        Ok(())
    }

    fn destroy(&mut self) -> Result<(), DisplayError> {
        self.destroyed = true;
        self.active_rects.clear();
        if let Some(x11) = &mut self.fallback_x11 {
            x11.destroy()?;
        }
        Ok(())
    }

    fn is_destroyed(&self) -> bool {
        self.destroyed
    }

    fn poll_events(&mut self) {
        if let Some(x11) = &mut self.fallback_x11 {
            x11.poll_events();
            self.bounds = x11.bounds();
        }
    }

    fn clear_damage(&mut self) {
        if let Some(x11) = &mut self.fallback_x11 {
            x11.clear_damage();
        }
    }
}
