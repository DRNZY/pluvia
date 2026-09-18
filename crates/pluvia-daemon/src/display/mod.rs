pub mod gnome_bridge;
pub mod layer_shell;
pub mod mock;
pub mod x11;

use pluvia_core::render::{AlphaHitMask, Rect};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Display and window backend errors.
#[derive(Debug, thiserror::Error)]
pub enum DisplayError {
    #[error("Surface has been destroyed")]
    SurfaceDestroyed,
    #[error("Display backend error: {0}")]
    Backend(String),
    #[error("Protocol error: {0}")]
    Protocol(String),
    #[error("Invalid geometry: {0}")]
    InvalidGeometry(String),
    #[error("D-Bus communication error: {0}")]
    DBus(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// 2D bounding box for desktop surfaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SurfaceBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl SurfaceBounds {
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x
            && px < self.x + self.width as i32
            && py >= self.y
            && py < self.y + self.height as i32
    }

    pub fn to_rect(&self) -> Rect {
        Rect::new(
            self.x as f64,
            self.y as f64,
            self.width as f64,
            self.height as f64,
        )
    }
}

/// Physical or virtual monitor screen geometry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScreenGeometry {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f64,
}

impl ScreenGeometry {
    pub fn new(
        name: impl Into<String>,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        scale_factor: f64,
    ) -> Self {
        Self {
            name: name.into(),
            x,
            y,
            width,
            height,
            scale_factor,
        }
    }

    pub fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x
            && px < self.x + self.width as i32
            && py >= self.y
            && py < self.y + self.height as i32
    }
}

/// Multi-monitor layout and coordinate translation engine.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct DisplayLayout {
    pub screens: Vec<ScreenGeometry>,
}

impl DisplayLayout {
    pub fn new(screens: Vec<ScreenGeometry>) -> Self {
        Self { screens }
    }

    pub fn primary_monitor(&self) -> Option<&ScreenGeometry> {
        self.screens.first()
    }

    pub fn find_monitor_at(&self, x: i32, y: i32) -> Option<&ScreenGeometry> {
        self.screens.iter().find(|s| s.contains(x, y))
    }

    pub fn find_monitor_by_name(&self, name: &str) -> Option<&ScreenGeometry> {
        self.screens.iter().find(|s| s.name == name)
    }

    /// Translates a relative coordinate offset (e.g. from monitor top-left) onto a target monitor.
    pub fn translate_relative(
        &self,
        relative_offset: (i32, i32),
        _source: &ScreenGeometry,
        target: &ScreenGeometry,
    ) -> (i32, i32) {
        (target.x + relative_offset.0, target.y + relative_offset.1)
    }

    /// Translates absolute global canvas coordinates from source monitor coordinates to target monitor coordinates.
    pub fn translate_point(
        &self,
        point: (i32, i32),
        source: &ScreenGeometry,
        target: &ScreenGeometry,
    ) -> (i32, i32) {
        let rel_x = point.0 - source.x;
        let rel_y = point.1 - source.y;
        (target.x + rel_x, target.y + rel_y)
    }
}

/// Anchor positioning type for desktop skin windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Anchor {
    TopLeft,
    TopCenter,
    TopRight,
    Center,
    BottomLeft,
    BottomCenter,
    BottomRight,
    Absolute,
}

impl Anchor {
    /// Resolves top-left screen coordinates (x, y) given screen geometry, widget size, and margins.
    ///
    /// Margins represent the inward offset from the respective anchored edges:
    /// - `TopLeft`: `(screen.x + margin_x, screen.y + margin_y)`
    /// - `TopCenter`: `(screen.x + (screen.width - w) / 2 + margin_x, screen.y + margin_y)`
    /// - `TopRight`: `(screen.x + screen.width - w - margin_x, screen.y + margin_y)`
    /// - `Center`: `(screen.x + (screen.width - w) / 2 + margin_x, screen.y + (screen.height - h) / 2 + margin_y)`
    /// - `BottomLeft`: `(screen.x + margin_x, screen.y + screen.height - h - margin_y)`
    /// - `BottomCenter`: `(screen.x + (screen.width - w) / 2 + margin_x, screen.y + screen.height - h - margin_y)`
    /// - `BottomRight`: `(screen.x + screen.width - w - margin_x, screen.y + screen.height - h - margin_y)`
    /// - `Absolute`: `(screen.x + margin_x, screen.y + margin_y)`
    pub fn resolve_position(
        &self,
        screen: &ScreenGeometry,
        widget_w: u32,
        widget_h: u32,
        margin_x: i32,
        margin_y: i32,
    ) -> (i32, i32) {
        let sw = screen.width as i32;
        let sh = screen.height as i32;
        let w = widget_w as i32;
        let h = widget_h as i32;

        match self {
            Anchor::TopLeft => (screen.x + margin_x, screen.y + margin_y),
            Anchor::TopCenter => (screen.x + ((sw - w) / 2) + margin_x, screen.y + margin_y),
            Anchor::TopRight => (screen.x + sw - w - margin_x, screen.y + margin_y),
            Anchor::Center => (
                screen.x + ((sw - w) / 2) + margin_x,
                screen.y + ((sh - h) / 2) + margin_y,
            ),
            Anchor::BottomLeft => (screen.x + margin_x, screen.y + sh - h - margin_y),
            Anchor::BottomCenter => {
                (screen.x + ((sw - w) / 2) + margin_x, screen.y + sh - h - margin_y)
            }
            Anchor::BottomRight => (
                screen.x + sw - w - margin_x,
                screen.y + sh - h - margin_y,
            ),
            Anchor::Absolute => (screen.x + margin_x, screen.y + margin_y),
        }
    }

    /// Resolves full `SurfaceBounds` given screen geometry, widget size, and margins.
    pub fn resolve_bounds(
        &self,
        screen: &ScreenGeometry,
        widget_w: u32,
        widget_h: u32,
        margin_x: i32,
        margin_y: i32,
    ) -> SurfaceBounds {
        let (x, y) = self.resolve_position(screen, widget_w, widget_h, margin_x, margin_y);
        SurfaceBounds::new(x, y, widget_w, widget_h)
    }
}

/// Supported desktop window backend types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BackendType {
    WlrLayerShell,
    GnomeWayland,
    X11,
    Mock,
}

impl BackendType {
    /// Detects the active desktop window backend from runtime environment variables.
    pub fn detect() -> Self {
        Self::detect_from_env(|k| std::env::var(k).ok())
    }

    /// Detects the backend using a provided map of environment variables (for testing).
    pub fn detect_from_map(map: &HashMap<String, String>) -> Self {
        Self::detect_from_env(|k| map.get(k).cloned())
    }

    /// Detects the backend using a custom variable resolver function.
    pub fn detect_from_env<F: Fn(&str) -> Option<String>>(lookup: F) -> Self {
        // 1. Explicit override check
        if let Some(explicit) = lookup("PLUVIA_BACKEND").or_else(|| lookup("BACKEND")) {
            let lower = explicit.trim().to_lowercase();
            match lower.as_str() {
                "mock" | "test" | "headless" => return BackendType::Mock,
                "layer-shell" | "layershell" | "wlr" | "wlr-layer-shell" | "wayland" => {
                    return BackendType::WlrLayerShell;
                }
                "gnome" | "gnome-wayland" | "gnome-shell" => return BackendType::GnomeWayland,
                "x11" | "xorg" | "x" => return BackendType::X11,
                _ => {}
            }
        }

        // 2. Wayland session check
        let wayland_display = lookup("WAYLAND_DISPLAY").filter(|s| !s.trim().is_empty());
        let xdg_session_type = lookup("XDG_SESSION_TYPE")
            .unwrap_or_default()
            .to_lowercase();
        let is_wayland = wayland_display.is_some() || xdg_session_type == "wayland";

        if is_wayland {
            let desktop = lookup("XDG_CURRENT_DESKTOP")
                .unwrap_or_default()
                .to_uppercase();
            let session_desktop = lookup("XDG_SESSION_DESKTOP")
                .unwrap_or_default()
                .to_uppercase();
            let gnome_session = lookup("GNOME_DESKTOP_SESSION_ID").is_some();

            if desktop.contains("GNOME") || session_desktop.contains("GNOME") || gnome_session {
                BackendType::GnomeWayland
            } else {
                BackendType::WlrLayerShell
            }
        } else if lookup("DISPLAY").filter(|s| !s.trim().is_empty()).is_some()
            || xdg_session_type == "x11"
        {
            BackendType::X11
        } else {
            // Default fallback
            BackendType::X11
        }
    }
}

/// Abstract desktop surface trait for managing window lifecycle, bounds, rendering,
/// visibility, and mouse click-through alpha hit-test masks.
pub trait DesktopSurface: Send + Sync {
    /// Unique identifier for this surface.
    fn id(&self) -> &str;

    /// The display backend powering this surface.
    fn backend_type(&self) -> BackendType;

    /// Current surface bounds on the display canvas.
    fn bounds(&self) -> SurfaceBounds;

    /// Updates the surface bounds.
    fn set_bounds(&mut self, bounds: SurfaceBounds) -> Result<(), DisplayError>;

    /// Returns true if the surface is currently visible.
    fn is_visible(&self) -> bool;

    /// Shows or hides the surface.
    fn set_visible(&mut self, visible: bool) -> Result<(), DisplayError>;

    /// Updates the rendered pixel contents from a Cairo image surface and submits
    /// the corresponding alpha hit-test mask for click-through input regions.
    fn update_surface(
        &mut self,
        surface: &cairo::ImageSurface,
        hit_mask: &AlphaHitMask,
    ) -> Result<(), DisplayError>;

    /// Updates the alpha hit-test mask without re-uploading surface pixel buffer.
    fn update_hit_mask(&mut self, hit_mask: &AlphaHitMask) -> Result<(), DisplayError>;

    /// Closes and destroys the surface.
    fn destroy(&mut self) -> Result<(), DisplayError>;

    /// Returns true if the surface has been destroyed.
    fn is_destroyed(&self) -> bool;
}
