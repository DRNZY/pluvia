use cairo::{Format, ImageSurface};
use pluvia_core::render::{AlphaHitMask, Rect};
use pluvia_daemon::display::gnome_bridge::{GnomeBridgeMode, GnomeBridgeSurface};
use pluvia_daemon::display::layer_shell::{Layer, LayerShellAnchor, LayerShellSurface};
use pluvia_daemon::display::mock::MockDesktopSurface;
use pluvia_daemon::display::x11::X11Surface;
use pluvia_daemon::display::{
    Anchor, BackendType, DesktopSurface, DisplayLayout, ScreenGeometry, SurfaceBounds,
};
use std::collections::HashMap;

#[test]
fn test_anchor_coordinate_resolution_standard_screens() {
    // 1. 1920x1080 viewport with widget 400x200 and margins (20, 30)
    let screen_fhd = ScreenGeometry::new("FHD-1", 0, 0, 1920, 1080, 1.0);
    let (w, h) = (400, 200);
    let (mx, my) = (20, 30);

    let b_tl = Anchor::TopLeft.resolve_bounds(&screen_fhd, w, h, mx, my);
    assert_eq!(b_tl, SurfaceBounds::new(20, 30, 400, 200));

    let b_tc = Anchor::TopCenter.resolve_bounds(&screen_fhd, w, h, mx, my);
    assert_eq!(b_tc, SurfaceBounds::new(780, 30, 400, 200));

    let b_tr = Anchor::TopRight.resolve_bounds(&screen_fhd, w, h, mx, my);
    assert_eq!(b_tr, SurfaceBounds::new(1500, 30, 400, 200));

    let b_c = Anchor::Center.resolve_bounds(&screen_fhd, w, h, mx, my);
    assert_eq!(b_c, SurfaceBounds::new(780, 470, 400, 200));

    let b_bl = Anchor::BottomLeft.resolve_bounds(&screen_fhd, w, h, mx, my);
    assert_eq!(b_bl, SurfaceBounds::new(20, 850, 400, 200));

    let b_bc = Anchor::BottomCenter.resolve_bounds(&screen_fhd, w, h, mx, my);
    assert_eq!(b_bc, SurfaceBounds::new(780, 850, 400, 200));

    let b_br = Anchor::BottomRight.resolve_bounds(&screen_fhd, w, h, mx, my);
    assert_eq!(b_br, SurfaceBounds::new(1500, 850, 400, 200));

    let b_abs = Anchor::Absolute.resolve_bounds(&screen_fhd, w, h, mx, my);
    assert_eq!(b_abs, SurfaceBounds::new(20, 30, 400, 200));

    // 2. 2560x1440 viewport with widget 500x300 and zero margin
    let screen_qhd = ScreenGeometry::new("QHD-1", 0, 0, 2560, 1440, 1.0);
    let (w2, h2) = (500, 300);
    assert_eq!(
        Anchor::TopCenter.resolve_bounds(&screen_qhd, w2, h2, 0, 0),
        SurfaceBounds::new(1030, 0, 500, 300)
    );
    assert_eq!(
        Anchor::TopRight.resolve_bounds(&screen_qhd, w2, h2, 0, 0),
        SurfaceBounds::new(2060, 0, 500, 300)
    );
    assert_eq!(
        Anchor::Center.resolve_bounds(&screen_qhd, w2, h2, 0, 0),
        SurfaceBounds::new(1030, 570, 500, 300)
    );
    assert_eq!(
        Anchor::BottomRight.resolve_bounds(&screen_qhd, w2, h2, 0, 0),
        SurfaceBounds::new(2060, 1140, 500, 300)
    );

    // 3. 5120x1440 ultrawide viewport with widget 600x250 and margins (50, 40)
    let screen_uw = ScreenGeometry::new("UW-1", 0, 0, 5120, 1440, 1.0);
    let (w3, h3) = (600, 250);
    let (mx3, my3) = (50, 40);
    assert_eq!(
        Anchor::TopCenter.resolve_bounds(&screen_uw, w3, h3, mx3, my3),
        SurfaceBounds::new(2310, 40, 600, 250)
    );
    assert_eq!(
        Anchor::TopRight.resolve_bounds(&screen_uw, w3, h3, mx3, my3),
        SurfaceBounds::new(4470, 40, 600, 250)
    );
    assert_eq!(
        Anchor::Center.resolve_bounds(&screen_uw, w3, h3, mx3, my3),
        SurfaceBounds::new(2310, 635, 600, 250)
    );
    assert_eq!(
        Anchor::BottomRight.resolve_bounds(&screen_uw, w3, h3, mx3, my3),
        SurfaceBounds::new(4470, 1150, 600, 250)
    );
}

#[test]
fn test_multi_monitor_coordinate_translation() {
    let mon1 = ScreenGeometry::new("DP-1", 0, 0, 1920, 1080, 1.0);
    let mon2 = ScreenGeometry::new("DP-2", 1920, 0, 2560, 1440, 1.0);
    let mon3 = ScreenGeometry::new("HDMI-1", 4480, 100, 1920, 1080, 1.0);

    let layout = DisplayLayout::new(vec![mon1.clone(), mon2.clone(), mon3.clone()]);
    assert_eq!(layout.screens.len(), 3);

    // Anchor BottomRight on Monitor 2 (offset 1920, 0, size 2560x1440)
    let bounds_m2 = Anchor::BottomRight.resolve_bounds(&mon2, 400, 200, 20, 20);
    assert_eq!(bounds_m2.x, 1920 + 2560 - 400 - 20); // 4060
    assert_eq!(bounds_m2.y, 0 + 1440 - 200 - 20); // 1220

    // Anchor Center on Monitor 3 (offset 4480, 100, size 1920x1080)
    let bounds_m3 = Anchor::Center.resolve_bounds(&mon3, 400, 200, 0, 0);
    assert_eq!(bounds_m3.x, 4480 + (1920 - 400) / 2); // 5240
    assert_eq!(bounds_m3.y, 100 + (1080 - 200) / 2); // 540

    // Coordinate translation between monitors:
    // Move relative position from Mon 1 (100, 100) to Mon 2
    let translated = layout.translate_relative((100, 100), &mon1, &mon2);
    assert_eq!(translated, (1920 + 100, 0 + 100));

    // Monitor lookup by coordinates
    assert_eq!(layout.find_monitor_at(500, 500).map(|s| s.name.as_str()), Some("DP-1"));
    assert_eq!(layout.find_monitor_at(2500, 500).map(|s| s.name.as_str()), Some("DP-2"));
    assert_eq!(layout.find_monitor_at(5000, 500).map(|s| s.name.as_str()), Some("HDMI-1"));
    assert_eq!(layout.find_monitor_at(10000, 10000), None);
}

#[test]
fn test_mock_desktop_surface_lifecycle_and_hit_mask() {
    let mut surface = MockDesktopSurface::new("clock-widget", SurfaceBounds::new(100, 100, 300, 200));

    assert_eq!(surface.id(), "clock-widget");
    assert_eq!(surface.backend_type(), BackendType::Mock);
    assert_eq!(surface.bounds(), SurfaceBounds::new(100, 100, 300, 200));
    assert!(surface.is_visible());
    assert!(!surface.is_destroyed());

    // Update visibility
    surface.set_visible(false).unwrap();
    assert!(!surface.is_visible());
    surface.set_visible(true).unwrap();
    assert!(surface.is_visible());

    // Update bounds
    surface.set_bounds(SurfaceBounds::new(150, 120, 320, 220)).unwrap();
    assert_eq!(surface.bounds(), SurfaceBounds::new(150, 120, 320, 220));

    // Create an ImageSurface and an AlphaHitMask with two disjoint interactive buttons
    let img_surface = ImageSurface::create(Format::ARgb32, 320, 220).unwrap();
    let disjoint_rects = vec![
        Rect::new(20.0, 20.0, 60.0, 30.0),
        Rect::new(150.0, 80.0, 80.0, 40.0),
    ];
    let hit_mask = AlphaHitMask::from_rectangles(320, 220, &disjoint_rects);

    surface.update_surface(&img_surface, &hit_mask).unwrap();
    assert_eq!(surface.update_count(), 1);
    assert_eq!(surface.damage_history().len(), 1);

    // Check submitted disjoint rectangles for click-through
    let submitted_rects = surface.hit_mask_rectangles();
    assert_eq!(submitted_rects.len(), 2);
    assert_eq!(submitted_rects[0], Rect::new(20.0, 20.0, 60.0, 30.0));
    assert_eq!(submitted_rects[1], Rect::new(150.0, 80.0, 80.0, 40.0));

    // Update hit mask independently
    let new_disjoint = vec![Rect::new(10.0, 10.0, 40.0, 40.0)];
    let new_mask = AlphaHitMask::from_rectangles(320, 220, &new_disjoint);
    surface.update_hit_mask(&new_mask).unwrap();
    assert_eq!(surface.hit_mask_rectangles().len(), 1);
    assert_eq!(surface.hit_mask_rectangles()[0], Rect::new(10.0, 10.0, 40.0, 40.0));

    // Destroy
    surface.destroy().unwrap();
    assert!(surface.is_destroyed());
    // Subsequent operations fail after destroy
    assert!(surface.set_visible(true).is_err());
    assert!(surface.update_surface(&img_surface, &hit_mask).is_err());
}

#[test]
fn test_backend_detection_environment_logic() {
    // 1. Pure Wayland (Sway / Hyprland)
    let mut env = HashMap::new();
    env.insert("WAYLAND_DISPLAY".to_string(), "wayland-1".to_string());
    env.insert("XDG_SESSION_TYPE".to_string(), "wayland".to_string());
    env.insert("XDG_CURRENT_DESKTOP".to_string(), "sway".to_string());
    assert_eq!(
        BackendType::detect_from_map(&env),
        BackendType::WlrLayerShell
    );

    // 2. GNOME on Wayland
    let mut gnome_env = HashMap::new();
    gnome_env.insert("WAYLAND_DISPLAY".to_string(), "wayland-0".to_string());
    gnome_env.insert("XDG_SESSION_TYPE".to_string(), "wayland".to_string());
    gnome_env.insert("XDG_CURRENT_DESKTOP".to_string(), "GNOME".to_string());
    assert_eq!(
        BackendType::detect_from_map(&gnome_env),
        BackendType::GnomeWayland
    );

    // Ubuntu variant: ubuntu:GNOME
    let mut ubuntu_env = HashMap::new();
    ubuntu_env.insert("WAYLAND_DISPLAY".to_string(), "wayland-0".to_string());
    ubuntu_env.insert("XDG_SESSION_TYPE".to_string(), "wayland".to_string());
    ubuntu_env.insert("XDG_CURRENT_DESKTOP".to_string(), "ubuntu:GNOME".to_string());
    assert_eq!(
        BackendType::detect_from_map(&ubuntu_env),
        BackendType::GnomeWayland
    );

    // 3. X11 session
    let mut x11_env = HashMap::new();
    x11_env.insert("DISPLAY".to_string(), ":0".to_string());
    x11_env.insert("XDG_SESSION_TYPE".to_string(), "x11".to_string());
    assert_eq!(BackendType::detect_from_map(&x11_env), BackendType::X11);

    // 4. Explicit override: PLUVIA_BACKEND
    let mut override_env = HashMap::new();
    override_env.insert("PLUVIA_BACKEND".to_string(), "mock".to_string());
    override_env.insert("WAYLAND_DISPLAY".to_string(), "wayland-1".to_string());
    assert_eq!(BackendType::detect_from_map(&override_env), BackendType::Mock);

    override_env.insert("PLUVIA_BACKEND".to_string(), "x11".to_string());
    assert_eq!(BackendType::detect_from_map(&override_env), BackendType::X11);

    override_env.insert("PLUVIA_BACKEND".to_string(), "layer-shell".to_string());
    assert_eq!(
        BackendType::detect_from_map(&override_env),
        BackendType::WlrLayerShell
    );

    override_env.insert("PLUVIA_BACKEND".to_string(), "gnome".to_string());
    assert_eq!(
        BackendType::detect_from_map(&override_env),
        BackendType::GnomeWayland
    );
}

#[test]
fn test_layer_shell_backend_configuration() {
    let bounds = SurfaceBounds::new(100, 50, 400, 300);
    let mut surface = LayerShellSurface::new(
        "weather-widget",
        bounds,
        Layer::Bottom,
        "DP-1",
        Anchor::TopRight,
        (20, 30),
    );

    assert_eq!(surface.id(), "weather-widget");
    assert_eq!(surface.backend_type(), BackendType::WlrLayerShell);
    assert_eq!(surface.layer(), Layer::Bottom);
    assert_eq!(surface.namespace(), "pluvia");
    assert_eq!(surface.output(), "DP-1");

    // Anchor translation to wlr-layer-shell bitflags
    let flags = surface.layer_shell_anchors();
    assert!(flags.contains(LayerShellAnchor::TOP));
    assert!(flags.contains(LayerShellAnchor::RIGHT));
    assert!(!flags.contains(LayerShellAnchor::BOTTOM));
    assert!(!flags.contains(LayerShellAnchor::LEFT));

    // Margins
    let (top, right, bottom, left) = surface.margins();
    assert_eq!((top, right, bottom, left), (30, 20, 0, 0));

    // Submit AlphaHitMask with disjoint rectangles to input region
    let img_surface = ImageSurface::create(Format::ARgb32, 400, 300).unwrap();
    let disjoint = vec![Rect::new(10.0, 10.0, 50.0, 50.0)];
    let mask = AlphaHitMask::from_rectangles(400, 300, &disjoint);

    surface.update_surface(&img_surface, &mask).unwrap();
    assert_eq!(surface.input_region_rects(), disjoint);
    assert_eq!(surface.damage_rects().len(), 1);
}

#[test]
fn test_x11_backend_window_hints_and_shape() {
    let bounds = SurfaceBounds::new(200, 150, 300, 200);
    let mut surface = X11Surface::new("system-widget", bounds, 0x123456);

    assert_eq!(surface.id(), "system-widget");
    assert_eq!(surface.backend_type(), BackendType::X11);
    assert_eq!(surface.window_id(), 0x123456);

    // EWMH atoms & properties
    assert_eq!(surface.window_type_atom(), "_NET_WM_WINDOW_TYPE_DESKTOP");
    assert!(surface.has_state_atom("_NET_WM_STATE_BELOW"));
    assert!(surface.has_state_atom("_NET_WM_STATE_STICKY"));
    assert_eq!(surface.desktop_atom_value(), 0xFFFFFFFF);

    // ShapeInput click-through submission
    let img_surface = ImageSurface::create(Format::ARgb32, 300, 200).unwrap();
    let disjoint = vec![
        Rect::new(10.0, 10.0, 100.0, 50.0),
        Rect::new(150.0, 80.0, 100.0, 50.0),
    ];
    let mask = AlphaHitMask::from_rectangles(300, 200, &disjoint);

    surface.update_surface(&img_surface, &mask).unwrap();
    assert_eq!(surface.shape_input_rects(), disjoint);
    assert_eq!(surface.damage_rects().len(), 1);
}

#[test]
fn test_gnome_bridge_extension_and_fallback() {
    let bounds = SurfaceBounds::new(50, 50, 350, 250);

    // 1. Extension mode
    let mut ext_surface = GnomeBridgeSurface::new_extension("gnome-widget", bounds);
    assert_eq!(ext_surface.backend_type(), BackendType::GnomeWayland);
    assert_eq!(ext_surface.mode(), GnomeBridgeMode::Extension);
    assert_eq!(ext_surface.extension_uuid(), "pluvia-shell@pluvia.org");
    assert_eq!(ext_surface.dbus_interface(), "org.pluvia.Shell");

    let img_surface = ImageSurface::create(Format::ARgb32, 350, 250).unwrap();
    let disjoint = vec![Rect::new(15.0, 15.0, 80.0, 30.0)];
    let mask = AlphaHitMask::from_rectangles(350, 250, &disjoint);

    ext_surface.update_surface(&img_surface, &mask).unwrap();
    assert_eq!(ext_surface.active_rects(), disjoint);

    // 2. Fallback mode to XWayland
    let mut fallback_surface = GnomeBridgeSurface::new_xwayland_fallback("gnome-fallback", bounds, 0xABCDEF);
    assert_eq!(fallback_surface.backend_type(), BackendType::GnomeWayland);
    assert_eq!(fallback_surface.mode(), GnomeBridgeMode::XWaylandFallback);
    assert!(fallback_surface.fallback_x11().is_some());

    fallback_surface.update_surface(&img_surface, &mask).unwrap();
    assert_eq!(fallback_surface.active_rects(), disjoint);
    assert_eq!(
        fallback_surface.fallback_x11().unwrap().window_type_atom(),
        "_NET_WM_WINDOW_TYPE_DESKTOP"
    );
}
