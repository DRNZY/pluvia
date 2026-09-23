use pluvia_core::extractor::{
    extract_rmskin_package, extract_rmskin_package_with_limit, pack_rmskin_package,
    ExtractionError,
};
use pluvia_core::vfs::VfsResolver;
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
fn test_vfs_case_folded_cache_hit() {
    let tmp = tempdir().unwrap();
    let skin_dir = tmp.path().join("Mond").join("Clock");
    fs::create_dir_all(&skin_dir).unwrap();
    let ini_file = skin_dir.join("Clock.ini");
    File::create(&ini_file).unwrap();

    let vfs = VfsResolver::new();

    // Resolve with mixed case
    let resolved = vfs.resolve(tmp.path(), "Mond\\Clock\\Clock.ini").unwrap();
    assert_eq!(resolved, ini_file);
    assert_eq!(vfs.cache_len(), 1);

    // Case-folded lookup for lowercase variant should already be cached
    assert!(vfs.is_cached(tmp.path(), "mond\\clock\\clock.ini"));

    // And resolving it directly hits the same cache entry without increasing cache_len
    let cached = vfs.resolve(tmp.path(), "mond\\clock\\clock.ini").unwrap();
    assert_eq!(cached, ini_file);
    assert_eq!(vfs.cache_len(), 1);
}

#[test]
fn test_vfs_base_jailing() {
    let tmp = tempdir().unwrap();
    let base = tmp.path().join("Skins");
    fs::create_dir_all(&base).unwrap();
    let clock_file = base.join("Clock.ini");
    File::create(&clock_file).unwrap();

    let vfs = VfsResolver::new();

    // Popping above base via .. must be jailed to base
    let resolved = vfs.resolve(&base, "../../Clock.ini");
    assert_eq!(resolved, Some(clock_file));

    // Arbitrary traversal out of base must return None
    let evil = vfs.resolve(&base, "../../../../etc/passwd");
    assert!(evil.is_none());
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
fn test_anti_zip_slip_windows_backslash_traversal() {
    let tmp = tempdir().unwrap();
    let zip_path = tmp.path().join("backslash_slip.zip");

    let file = File::create(&zip_path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();

    // Windows backslash traversal entry
    zip.start_file("..\\..\\evil.txt", options).unwrap();
    zip.write_all(b"malicious payload").unwrap();
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
fn test_directory_symlink_rejection() {
    let tmp = tempdir().unwrap();
    let dest = tmp.path().join("extracted");
    fs::create_dir_all(&dest).unwrap();

    // Pre-create a directory symlink in dest targeting another dir
    let real_dir = tmp.path().join("outside");
    fs::create_dir_all(&real_dir).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&real_dir, dest.join("sym_dir")).unwrap();

    // Archive has a directory entry for sym_dir/
    let zip_path = tmp.path().join("dir_sym.zip");
    let file = File::create(&zip_path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();

    zip.add_directory("sym_dir/", options).unwrap();
    zip.finish().unwrap();

    let err = extract_rmskin_package(&zip_path, &dest).unwrap_err();
    assert!(matches!(err, ExtractionError::SymlinkForbidden));
}

#[test]
fn test_hardlink_target_forbidden() {
    let tmp = tempdir().unwrap();
    let dest = tmp.path().join("extracted");
    fs::create_dir_all(&dest).unwrap();

    // Pre-create a hardlinked file in dest
    let shared = tmp.path().join("shared.txt");
    fs::write(&shared, b"original").unwrap();
    let target = dest.join("hardlinked.txt");
    fs::hard_link(&shared, &target).unwrap();

    let zip_path = tmp.path().join("hardlink.zip");
    let file = File::create(&zip_path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();

    zip.start_file("hardlinked.txt", options).unwrap();
    zip.write_all(b"overwrite attempt").unwrap();
    zip.finish().unwrap();

    let err = extract_rmskin_package(&zip_path, &dest).unwrap_err();
    assert!(matches!(err, ExtractionError::HardlinkForbidden));
}

#[test]
fn test_quota_exceeded_zip_bomb() {
    let tmp = tempdir().unwrap();
    let dest = tmp.path().join("extracted");
    let zip_path = tmp.path().join("bomb.zip");

    let file = File::create(&zip_path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();

    let large_data = vec![b'A'; 2048];
    zip.start_file("large.bin", options).unwrap();
    zip.write_all(&large_data).unwrap();
    zip.finish().unwrap();

    // Enforce small quota of 500 bytes (lower than 2048 bytes)
    let err = extract_rmskin_package_with_limit(&zip_path, &dest, 500).unwrap_err();
    assert!(matches!(err, ExtractionError::QuotaExceeded));

    // Ensure the partial file was deleted
    assert!(!dest.join("large.bin").exists());
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

#[test]
fn test_pack_and_extract_roundtrip() {
    let tmp = tempdir().unwrap();
    let skin_root = tmp.path().join("SourceSkin");
    let res_dir = skin_root.join("@Resources").join("Images");
    fs::create_dir_all(&res_dir).unwrap();

    let ini_content = b"[Rainmeter]\nUpdate=500\n[MeterText]\nMeter=String\nText=Pluvia";
    fs::write(skin_root.join("Skin.ini"), ini_content).unwrap();

    let img_content = b"PNG_FAKE_IMAGE_DATA_12345";
    fs::write(res_dir.join("bg.png"), img_content).unwrap();

    let output_rmskin = tmp.path().join("packaged.rmskin");
    let pack_report = pack_rmskin_package(&skin_root, &output_rmskin).unwrap();

    assert_eq!(pack_report.files_packaged, 2);
    assert_eq!(
        pack_report.total_uncompressed_bytes,
        (ini_content.len() + img_content.len()) as u64
    );
    assert!(output_rmskin.exists());
    assert!(pack_report.package_size > 0);

    let unpack_dir = tmp.path().join("unpacked");
    let extract_report = extract_rmskin_package(&output_rmskin, &unpack_dir).unwrap();

    assert_eq!(extract_report.files_extracted, 2);
    assert_eq!(fs::read(unpack_dir.join("Skin.ini")).unwrap(), ini_content);
    assert_eq!(
        fs::read(unpack_dir.join("@Resources/Images/bg.png")).unwrap(),
        img_content
    );
}
