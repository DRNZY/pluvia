use pluvia_core::ini::{parse_skin_file, SkinConfig};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct DiscoveredSkin {
    pub suite: String,
    pub name: String,
    pub path: PathBuf,
    pub config: Option<SkinConfig>,
}

pub fn get_skins_directories() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    // 1. Current working directory ./skins
    if let Ok(cwd) = std::env::current_dir() {
        let local_skins = cwd.join("skins");
        if local_skins.is_dir() {
            dirs.push(local_skins);
        }
    }

    // 2. ~/.config/pluvia/skins
    if let Ok(home) = std::env::var("HOME") {
        let config_skins = PathBuf::from(&home).join(".config/pluvia/skins");
        if config_skins.is_dir() {
            dirs.push(config_skins);
        }
        let data_skins = PathBuf::from(&home).join(".local/share/pluvia/skins");
        if data_skins.is_dir() {
            dirs.push(data_skins);
        }
    }

    dirs
}

pub fn scan_installed_skins() -> BTreeMap<String, Vec<DiscoveredSkin>> {
    let mut suites: BTreeMap<String, Vec<DiscoveredSkin>> = BTreeMap::new();
    let dirs = get_skins_directories();

    for dir in dirs {
        scan_directory(&dir, &dir, &mut suites);
    }

    suites
}

fn scan_directory(
    root: &Path,
    current: &Path,
    suites: &mut BTreeMap<String, Vec<DiscoveredSkin>>,
) {
    let entries = match std::fs::read_dir(current) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Ignore @Resources folder
            if path.file_name().map(|n| n.to_string_lossy().starts_with('@')).unwrap_or(false) {
                continue;
            }
            scan_directory(root, &path, suites);
        } else if path.is_file() {
            let is_ini = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("ini"))
                .unwrap_or(false);

            if is_ini {
                let rel = path.strip_prefix(root).unwrap_or(&path);
                let suite = rel
                    .iter()
                    .next()
                    .map(|c| c.to_string_lossy().to_string())
                    .unwrap_or_else(|| "General".to_string());

                let name = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "Skin".to_string());

                let config = parse_skin_file(&path).ok();

                let discovered = DiscoveredSkin {
                    suite: suite.clone(),
                    name,
                    path: path.clone(),
                    config,
                };

                suites.entry(suite).or_default().push(discovered);
            }
        }
    }
}
