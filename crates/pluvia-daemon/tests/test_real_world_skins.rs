use pluvia_daemon::display::BackendType;
use pluvia_daemon::runtime::SkinRuntime;
use std::fs;
use std::path::{Path, PathBuf};

fn collect_skin_ini_files(dir: &Path, acc: &mut Vec<PathBuf>) {
    if !dir.exists() {
        return;
    }
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with('.') {
                        continue;
                    }
                }
                collect_skin_ini_files(&path, acc);
            } else if path.extension().map_or(false, |ext| ext.eq_ignore_ascii_case("ini")) {
                let path_str = path.to_string_lossy().to_lowercase();
                if path_str.contains("@resources") || path_str.contains("rmskin.ini") {
                    continue;
                }
                acc.push(path);
            }
        }
    }
}

#[test]
fn test_parse_and_tick_all_real_world_skins() {
    let mut runtime = SkinRuntime::new(BackendType::Mock);

    let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();

    let mut skin_files = Vec::new();

    // 1. Fixture real-world skins
    let fixtures_dir = base_dir.join("tests").join("fixtures").join("real_world_skins");
    collect_skin_ini_files(&fixtures_dir, &mut skin_files);

    // 2. Local config skins
    if let Ok(home) = std::env::var("HOME") {
        let config_skins = PathBuf::from(home).join(".config").join("pluvia").join("skins");
        collect_skin_ini_files(&config_skins, &mut skin_files);
    }

    // 3. Repository skins
    let repo_skins = base_dir.join("skins");
    collect_skin_ini_files(&repo_skins, &mut skin_files);

    assert!(!skin_files.is_empty(), "No skin files discovered for testing!");

    let mut loaded_count = 0;
    let mut failed_skins = Vec::new();
    let test_audio = vec![0.5f32; 1024];

    for skin_path in &skin_files {
        let mut skin_runtime = SkinRuntime::new(BackendType::Mock);
        match skin_runtime.load_skin(skin_path) {
            Ok(info) => {
                skin_runtime.feed_audio_samples(&test_audio);
                if let Err(e) = skin_runtime.tick_all() {
                    failed_skins.push((skin_path.clone(), format!("Tick failed: {:?}", e)));
                } else {
                    loaded_count += 1;
                }
            }
            Err(e) => {
                failed_skins.push((skin_path.clone(), format!("Load failed: {:?}", e)));
            }
        }
    }

    println!("\nSuccessfully verified {}/{} skins individually.", loaded_count, skin_files.len());

    if !failed_skins.is_empty() {
        eprintln!("\nFailed skins ({}):", failed_skins.len());
        for (path, err) in &failed_skins {
            eprintln!("  - {}: {}", path.display(), err);
        }
    }

    assert!(
        failed_skins.is_empty(),
        "Some skins failed to load or parse: {} failures out of {} skins",
        failed_skins.len(),
        skin_files.len()
    );
}
