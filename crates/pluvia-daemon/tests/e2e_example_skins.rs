use pluvia_daemon::display::BackendType;
use pluvia_daemon::runtime::SkinRuntime;
use std::path::PathBuf;

#[test]
fn test_load_and_tick_all_example_skins() {
    let mut runtime = SkinRuntime::new(BackendType::Mock);

    let skin_paths = vec![
        "skins/Mond/Clock/Clock.ini",
        "skins/Monterey/System/System.ini",
        "skins/Monterey/Player/Player.ini",
        "skins/Flint/Visualizer/Visualizer.ini",
        "skins/Silicon/TelemetryHUD/TelemetryHUD.ini",
    ];

    for rel_path in &skin_paths {
        let full_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join(rel_path);

        assert!(full_path.exists(), "Skin file not found: {}", full_path.display());

        let info = runtime.load_skin(&full_path).unwrap_or_else(|e| {
            panic!("Failed to load skin {}: {:?}", rel_path, e);
        });

        assert!(info.meters_count > 0, "Skin {} has 0 meters", rel_path);
        assert!(info.bounds.width > 0, "Skin {} has 0 width", rel_path);
        assert!(info.bounds.height > 0, "Skin {} has 0 height", rel_path);
    }

    assert_eq!(runtime.list_skins().len(), 5);

    // Tick all skins
    for _ in 0..5 {
        runtime.tick_all().expect("tick_all failed");
    }

    // Verify Monterey Player interactive click
    let handled = runtime
        .handle_mouse_click("Monterey/Player", 160.0, 120.0, 1)
        .expect("Failed to handle play button click");
    assert!(handled, "Play button click in Monterey/Player was not handled");

    // Feed mock audio buffer to visualizers
    let test_audio = vec![0.5f32; 1024];
    runtime.feed_audio_samples(&test_audio);
    runtime.tick_all().expect("tick_all after audio failed");
}
