use gtk4::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;
use libadwaita::prelude::*;
use pluvia_core::extractor::extract_rmskin_package;
use std::path::{Path, PathBuf};

pub fn open_package_installer<F: Fn() + 'static>(parent: &adw::ApplicationWindow, on_success: F) {
    let dialog = gtk::FileDialog::builder()
        .title("Select Rainmeter Skin Package (.rmskin)")
        .modal(true)
        .build();

    let filter = gtk::FileFilter::new();
    filter.set_name(Some("Rainmeter Skin Package (*.rmskin, *.zip)"));
    filter.add_pattern("*.rmskin");
    filter.add_pattern("*.zip");

    let filters = gtk::gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&filter);
    dialog.set_filters(Some(&filters));

    let parent_clone = parent.clone();
    dialog.open(Some(parent), gtk::gio::Cancellable::NONE, move |res| {
        if let Ok(file) = res {
            if let Some(path) = file.path() {
                install_package(&parent_clone, &path, &on_success);
            }
        }
    });
}

fn install_package<F: Fn()>(parent: &adw::ApplicationWindow, package_path: &Path, on_success: &F) {
    let dest_dir = if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".config/pluvia/skins")
    } else {
        PathBuf::from("./skins")
    };

    let _ = std::fs::create_dir_all(&dest_dir);

    match extract_rmskin_package(package_path, &dest_dir) {
        Ok(report) => {
            let alert = adw::AlertDialog::builder()
                .heading("Skin Package Installed")
                .body(&format!(
                    "Successfully extracted {} files ({} bytes) to:\n{}",
                    report.files_extracted,
                    report.total_bytes,
                    dest_dir.display()
                ))
                .build();
            alert.add_response("ok", "OK");
            alert.present(Some(parent));
            on_success();
        }
        Err(e) => {
            let alert = adw::AlertDialog::builder()
                .heading("Installation Failed")
                .body(&format!("Failed to install package:\n{}", e))
                .build();
            alert.add_response("ok", "OK");
            alert.present(Some(parent));
        }
    }
}
