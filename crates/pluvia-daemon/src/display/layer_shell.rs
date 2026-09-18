use super::{Anchor, BackendType, DesktopSurface, DisplayError, SurfaceBounds};
use pluvia_core::render::{AlphaHitMask, Rect};
use serde::{Deserialize, Serialize};

/// Wayland wlr-layer-shell surface layers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Layer {
    Background = 0,
    Bottom = 1,
    Top = 2,
    Overlay = 3,
}

/// Bitflags representing zwlr_layer_surface_v1 edge anchors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LayerShellAnchor(u32);

impl LayerShellAnchor {
    pub const TOP: Self = Self(1);
    pub const BOTTOM: Self = Self(2);
    pub const LEFT: Self = Self(4);
    pub const RIGHT: Self = Self(8);

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    pub const fn bits(&self) -> u32 {
        self.0
    }

    pub const fn from_bits_truncate(bits: u32) -> Self {
        Self(bits)
    }
}

impl std::ops::BitOr for LayerShellAnchor {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// Desktop surface backed by Wayland `wlr-layer-shell`.
#[derive(Debug, Clone)]
pub struct LayerShellSurface {
    id: String,
    bounds: SurfaceBounds,
    layer: Layer,
    output: String,
    anchor: Anchor,
    margin: (i32, i32),
    namespace: String,
    visible: bool,
    destroyed: bool,
    input_region_rects: Vec<Rect>,
    damage_rects: Vec<Rect>,
}

impl LayerShellSurface {
    pub fn new(
        id: impl Into<String>,
        bounds: SurfaceBounds,
        layer: Layer,
        output: impl Into<String>,
        anchor: Anchor,
        margin: (i32, i32),
    ) -> Self {
        Self {
            id: id.into(),
            bounds,
            layer,
            output: output.into(),
            anchor,
            margin,
            namespace: "pluvia".to_string(),
            visible: true,
            destroyed: false,
            input_region_rects: Vec::new(),
            damage_rects: Vec::new(),
        }
    }

    pub fn layer(&self) -> Layer {
        self.layer
    }

    pub fn output(&self) -> &str {
        &self.output
    }

    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    pub fn set_namespace(&mut self, namespace: impl Into<String>) {
        self.namespace = namespace.into();
    }

    /// Converts the configured `Anchor` to `LayerShellAnchor` flags.
    pub fn layer_shell_anchors(&self) -> LayerShellAnchor {
        match self.anchor {
            Anchor::TopLeft => LayerShellAnchor::TOP | LayerShellAnchor::LEFT,
            Anchor::TopCenter => LayerShellAnchor::TOP,
            Anchor::TopRight => LayerShellAnchor::TOP | LayerShellAnchor::RIGHT,
            Anchor::Center => LayerShellAnchor::empty(),
            Anchor::BottomLeft => LayerShellAnchor::BOTTOM | LayerShellAnchor::LEFT,
            Anchor::BottomCenter => LayerShellAnchor::BOTTOM,
            Anchor::BottomRight => LayerShellAnchor::BOTTOM | LayerShellAnchor::RIGHT,
            Anchor::Absolute => LayerShellAnchor::TOP | LayerShellAnchor::LEFT,
        }
    }

    /// Returns the pixel margins (top, right, bottom, left) calculated from the anchor and margin offsets.
    pub fn margins(&self) -> (i32, i32, i32, i32) {
        let (mx, my) = self.margin;
        match self.anchor {
            Anchor::TopLeft => (my, 0, 0, mx),
            Anchor::TopCenter => (my, 0, 0, mx),
            Anchor::TopRight => (my, mx, 0, 0),
            Anchor::Center => (my, 0, 0, mx),
            Anchor::BottomLeft => (0, 0, my, mx),
            Anchor::BottomCenter => (0, 0, my, mx),
            Anchor::BottomRight => (0, mx, my, 0),
            Anchor::Absolute => (my, 0, 0, mx),
        }
    }

    pub fn input_region_rects(&self) -> &[Rect] {
        &self.input_region_rects
    }

    pub fn damage_rects(&self) -> &[Rect] {
        &self.damage_rects
    }
}

impl DesktopSurface for LayerShellSurface {
    fn id(&self) -> &str {
        &self.id
    }

    fn backend_type(&self) -> BackendType {
        BackendType::WlrLayerShell
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
        self.input_region_rects = hit_mask.to_rectangles();
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
        self.input_region_rects = hit_mask.to_rectangles();
        Ok(())
    }

    fn destroy(&mut self) -> Result<(), DisplayError> {
        self.destroyed = true;
        self.input_region_rects.clear();
        Ok(())
    }

    fn is_destroyed(&self) -> bool {
        self.destroyed
    }
}
