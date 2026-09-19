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
            let dir_name = path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
            // Ignore @Resources, plugins, .git, hidden directories, and Rainmeter-internal Extras folders
            if dir_name.starts_with('@') || dir_name.starts_with('.') || dir_name.eq_ignore_ascii_case("plugins")
                || dir_name.eq_ignore_ascii_case("extras")
                || dir_name.eq_ignore_ascii_case("@backup")
            {
                continue;
            }
            scan_directory(root, &path, suites);
        } else if path.is_file() {
            let filename = path.file_name().map(|n| n.to_string_lossy()).unwrap_or_default();
            let lower_fname = filename.to_ascii_lowercase();
            // Strip the .ini extension for stem-based comparison
            let lower_stem = lower_fname.strip_suffix(".ini").unwrap_or(&lower_fname);

            // Filter out RMSKIN package metadata and internal helper/settings dialogs
            // These exist at any folder depth inside a skin suite
            let is_internal = matches!(
                lower_stem,
                "rmskin"
                | "settings"
                | "whatsnew"
                | "whats_new"
                | "whats new"
                | "themeupdater"
                | "theme_updater"
                | "autolowpowermode"
                | "auto_low_power_mode"
                | "unlock"
                | "error"
                | "sidebar"
                | "updater"
                | "readme"
                | "changelog"
            );

            if is_internal || lower_fname == "rmskin.bmp" {
                continue;
            }

            let is_ini = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("ini"))
                .unwrap_or(false);

            if is_ini {
                let rel = path.strip_prefix(root).unwrap_or(&path);
                let components: Vec<String> = rel.iter().map(|c| c.to_string_lossy().to_string()).collect();
                let suite = components.first().cloned().unwrap_or_else(|| "General".to_string());

                let name = if components.len() > 2 {
                    let sub = &components[1..components.len() - 1];
                    let stem = path
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "Skin".to_string());
                    format!("{} / {}", sub.join(" / "), stem)
                } else {
                    path.file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "Skin".to_string())
                };

                // Only include skins that parse cleanly and contain at least one visual meter
                let config = match parse_skin_file(&path) {
                    Ok(cfg) if !cfg.meters.is_empty() => Some(cfg),
                    _ => continue,
                };

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
