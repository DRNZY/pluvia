use cairo::{Format, ImageSurface};
use pluvia_core::ini::{MeterConfig, SkinConfig};
use pluvia_core::render::meter_renderer::{MeterRenderer, SkinState};
use std::collections::HashMap;

#[test]
fn test_cairo_container_masking() {
    let mut config = SkinConfig::default();

    // 1. Container shape meter (50x50 square)
    let mut container = MeterConfig::default();
    container.name = "MainContainer".to_string();
    container.meter_type = "Shape".to_string();
    container.properties.insert("shape".to_string(), "Rectangle 0,0,50,50".to_string());
    container.properties.insert("shape.fill".to_string(), "Color 255,255,255,255".to_string());
    config.meters.insert("maincontainer".to_string(), container);
    config.meter_order.push("maincontainer".to_string());

    // 2. Child meter (100x100 square) masked by MainContainer
    let mut child = MeterConfig::default();
    child.name = "ChildGrid".to_string();
    child.meter_type = "Image".to_string();
    child.solid_color = Some("255,0,0,255".to_string());
    child.w = Some(100.0);
    child.h = Some(100.0);
    child.properties.insert("container".to_string(), "MainContainer".to_string());
    config.meters.insert("childgrid".to_string(), child);
    config.meter_order.push("childgrid".to_string());

    let state = SkinState::new(config, HashMap::new());
    let renderer = MeterRenderer::new();
    let surface = ImageSurface::create(Format::ARgb32, 200, 200).expect("surface creation");

    let hit_mask = renderer.render_to_surface(&state, &surface).expect("render should succeed");

    // The child meter should be rendered and masked to 50x50
    // A point at (25, 25) inside container should be opaque (true)
    assert!(hit_mask.contains(25, 25));

    // A point at (75, 75) outside the 50x50 container should NOT be opaque (false)
    assert!(!hit_mask.contains(75, 75));
}
