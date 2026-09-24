use pluvia_daemon::display::BackendType;
use pluvia_daemon::runtime::SkinRuntime;
use std::path::PathBuf;

#[test]
fn test_cleartext_runtime_e2e() {
    let mut runtime = SkinRuntime::new(BackendType::Mock);

    let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();

    let skin_paths = [
        base_dir.join("tests/fixtures/real_world_skins/cleartext/Cleartext Pure.ini"),
        base_dir.join("tests/fixtures/real_world_skins/cleartext/Cleartext.ini"),
        base_dir.join("tests/fixtures/real_world_skins/monstercat/visualizer.ini"),
    ];

    for path in &skin_paths {
        if path.exists() {
            let info = runtime.load_skin(path).expect("Failed to load skin");
            assert!(info.meters_count > 0);
            assert!(info.bounds.width > 0);
            assert!(info.bounds.height > 0);
        }
    }

    let fake_audio = vec![0.5f32; 1024];
    for _ in 0..10 {
        runtime.feed_audio_samples(&fake_audio);
        runtime.tick_all().expect("Runtime tick failed");
    }
}
