use pluvia_core::vfs::VfsResolver;
use pluvia_core::extractor::{extract_rmskin_package, ExtractionError};
use std::fs::{self, File};
use std::io::Write;
use tempfile::tempdir;

#[test]
fn test_vfs_case_insensitive_and_backslash_resolution() {
    let tmp = tempdir().unwrap();
    let res_dir = tmp.path().join("@Resources").join("Fonts");
    fs::create_dir_all(&res_dir).unwrap();
    let font_file = res_dir.join("Anurati.otf");
    File::create(&font_file).unwrap();

    let vfs = VfsResolver::new();
    // Resolving Windows-style backslashes and lowercase segments
    let resolved = vfs.resolve(tmp.path(), "@resources\\fonts\\anurati.otf");
    assert!(resolved.is_some());
    assert_eq!(resolved.unwrap(), font_file);

    // Drive letter stripping
    let resolved_drive = vfs.resolve(tmp.path(), "C:\\@resources\\fonts\\anurati.otf");
    assert!(resolved_drive.is_some());
    assert_eq!(resolved_drive.unwrap(), font_file);
}

#[test]
fn test_vfs_cached_lookup_verification() {
    let tmp = tempdir().unwrap();
    let skin_dir = tmp.path().join("Mond").join("Clock");
    fs::create_dir_all(&skin_dir).unwrap();
    let ini_file = skin_dir.join("Clock.ini");
    File::create(&ini_file).unwrap();

    let vfs = VfsResolver::new();
    let rel = "mond\\clock\\clock.ini";

    assert!(!vfs.is_cached(tmp.path(), rel));
    assert_eq!(vfs.cache_len(), 0);

    let resolved = vfs.resolve(tmp.path(), rel).unwrap();
    assert_eq!(resolved, ini_file);

    assert!(vfs.is_cached(tmp.path(), rel));
    assert_eq!(vfs.cache_len(), 1);

    // Delete the file from disk - cached lookup must still return the resolved path
    fs::remove_file(&ini_file).unwrap();
    assert!(!ini_file.exists());

    let cached_resolved = vfs.resolve(tmp.path(), rel).unwrap();
    assert_eq!(cached_resolved, ini_file);
}

#[test]
fn test_anti_zip_slip_path_traversal() {
    let tmp = tempdir().unwrap();
    let zip_path = tmp.path().join("malicious.zip");

    // Construct zip with traversal entry
    let file = File::create(&zip_path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();

    zip.start_file("../etc/passwd", options).unwrap();
    zip.write_all(b"root:x:0:0:root").unwrap();
    zip.finish().unwrap();

    let dest = tmp.path().join("extracted");
    let err = extract_rmskin_package(&zip_path, &dest).unwrap_err();
    assert!(matches!(err, ExtractionError::ZipSlipDetected));
}

#[test]
fn test_strict_symlink_rejection() {
    let tmp = tempdir().unwrap();
    let zip_path = tmp.path().join("symlink.zip");

    let file = File::create(&zip_path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();

    zip.add_symlink("evil_symlink", "/etc/passwd", options).unwrap();
    zip.finish().unwrap();

    let dest = tmp.path().join("extracted");
    let err = extract_rmskin_package(&zip_path, &dest).unwrap_err();
    assert!(matches!(err, ExtractionError::SymlinkForbidden));
}

#[test]
fn test_valid_zip_extraction_and_report() {
    let tmp = tempdir().unwrap();
    let zip_path = tmp.path().join("package.rmskin");

    let file = File::create(&zip_path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();

    let content1 = b"[Rainmeter]\nUpdate=1000\n";
    zip.start_file("Skins/Mond/Clock.ini", options).unwrap();
    zip.write_all(content1).unwrap();

    let content2 = b"ANURATI_OTF_BINARY_DATA";
    zip.start_file("Skins/Mond/@Resources/Fonts/Anurati.otf", options).unwrap();
    zip.write_all(content2).unwrap();

    zip.finish().unwrap();

    let dest = tmp.path().join("extracted");
    let report = extract_rmskin_package(&zip_path, &dest).unwrap();

    assert_eq!(report.files_extracted, 2);
    assert_eq!(report.total_bytes, (content1.len() + content2.len()) as u64);

    let extracted_ini = dest.join("Skins/Mond/Clock.ini");
    let extracted_font = dest.join("Skins/Mond/@Resources/Fonts/Anurati.otf");

    assert!(extracted_ini.exists());
    assert!(extracted_font.exists());
    assert_eq!(fs::read(&extracted_ini).unwrap(), content1);
    assert_eq!(fs::read(&extracted_font).unwrap(), content2);
}
