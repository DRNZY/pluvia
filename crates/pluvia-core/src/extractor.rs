use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use thiserror::Error;

/// Maximum allowable cumulative decompressed extraction size (500 MB) to prevent zip-bomb DoS attacks.
pub const MAX_EXTRACTION_BYTES: u64 = 500 * 1024 * 1024;

/// Errors that can occur during `.rmskin` package extraction.
#[derive(Error, Debug)]
pub enum ExtractionError {
    #[error("Zip-Slip directory traversal detected")]
    ZipSlipDetected,
    #[error("Symlink extraction is strictly forbidden")]
    SymlinkForbidden,
    #[error("Hardlink target mutation is strictly forbidden")]
    HardlinkForbidden,
    #[error("Extraction size quota exceeded")]
    QuotaExceeded,
    #[error("Archive error: {0}")]
    ArchiveError(#[from] zip::result::ZipError),
    #[error("IO error: {0}")]
    IoError(#[from] io::Error),
}

/// Statistics and metadata about a completed extraction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractionReport {
    pub files_extracted: usize,
    pub total_bytes: u64,
}

/// Extracts a Rainmeter `.rmskin` (or standard `.zip`) package to a target directory.
///
/// Security constraints:
/// - Absolute anti-Zip-Slip enforcement (normalizes Windows `\` backslashes before component
///   inspection, rejects parent directory / root components, and validates canonical destination prefixes).
/// - Strict symlink immunity (rejects symlink archive entries and existing symlinks on target paths/dirs).
/// - Hardlink safety (rejects writing to existing files with `nlink > 1` on Unix to prevent mutating shared inodes).
/// - Decompression quota enforcement (`MAX_EXTRACTION_BYTES` cap protecting against zip bomb DoS).
pub fn extract_rmskin_package<P: AsRef<Path>, Q: AsRef<Path>>(
    archive_path: P,
    dest_root: Q,
) -> Result<ExtractionReport, ExtractionError> {
    extract_rmskin_package_with_limit(archive_path, dest_root, MAX_EXTRACTION_BYTES)
}

/// Extracts a Rainmeter `.rmskin` package with a custom byte quota.
pub fn extract_rmskin_package_with_limit<P: AsRef<Path>, Q: AsRef<Path>>(
    archive_path: P,
    dest_root: Q,
    max_bytes: u64,
) -> Result<ExtractionReport, ExtractionError> {
    let dest_root = dest_root.as_ref();
    fs::create_dir_all(dest_root)?;
    let dest_canonical = dest_root.canonicalize()?;

    let file = File::open(archive_path)?;
    let mut zip = zip::ZipArchive::new(file)?;

    let mut files_extracted = 0;
    let mut total_bytes: u64 = 0;

    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;

        // 1. Strict symlink check via unix_mode & is_symlink
        if entry.is_symlink()
            || entry
                .unix_mode()
                .is_some_and(|mode| (mode & 0o170000) == 0o120000)
        {
            return Err(ExtractionError::SymlinkForbidden);
        }

        // 2. Component inspection on raw entry name with backslash normalization
        let raw_name = entry.name();
        let normalized = raw_name.replace('\\', "/");
        let norm_path = Path::new(&normalized);
        if norm_path.components().any(|c| {
            matches!(
                c,
                Component::ParentDir | Component::Prefix(_) | Component::RootDir
            )
        }) {
            return Err(ExtractionError::ZipSlipDetected);
        }

        // 3. Zip enclosed_name verification
        let enclosed = match entry.enclosed_name() {
            Some(path) => path.to_path_buf(),
            None => return Err(ExtractionError::ZipSlipDetected),
        };

        if enclosed.components().any(|c| {
            matches!(
                c,
                Component::ParentDir | Component::Prefix(_) | Component::RootDir
            )
        }) {
            return Err(ExtractionError::ZipSlipDetected);
        }

        // Validate each path component does not encounter pre-existing symlinks
        let mut check_path = dest_canonical.to_path_buf();
        for comp in enclosed.components() {
            check_path.push(comp);
            let check_str = check_path.to_string_lossy();
            let clean_check = Path::new(check_str.trim_end_matches('/'));
            if let Ok(meta) = fs::symlink_metadata(clean_check) {
                if meta.file_type().is_symlink() {
                    return Err(ExtractionError::SymlinkForbidden);
                }
            }
        }

        let raw_target = dest_canonical.join(&enclosed);
        let target_str = raw_target.to_string_lossy();
        let target_path = PathBuf::from(target_str.trim_end_matches('/'));

        // 4. Directory entry extraction
        if entry.is_dir() {
            if let Ok(meta) = fs::symlink_metadata(&target_path) {
                if meta.file_type().is_symlink() {
                    return Err(ExtractionError::SymlinkForbidden);
                }
            }
            fs::create_dir_all(&target_path)?;
            let canonical = target_path.canonicalize()?;
            if !canonical.starts_with(&dest_canonical) {
                return Err(ExtractionError::ZipSlipDetected);
            }
        } else {
            // 5. File entry extraction
            // Quota check based on declared uncompressed size
            let declared_size = entry.size();
            if total_bytes.saturating_add(declared_size) > max_bytes {
                return Err(ExtractionError::QuotaExceeded);
            }

            if let Some(parent) = target_path.parent() {
                if let Ok(parent_meta) = fs::symlink_metadata(parent) {
                    if parent_meta.file_type().is_symlink() {
                        return Err(ExtractionError::SymlinkForbidden);
                    }
                }
                fs::create_dir_all(parent)?;
                let canonical_parent = parent.canonicalize()?;
                if !canonical_parent.starts_with(&dest_canonical) {
                    return Err(ExtractionError::ZipSlipDetected);
                }
            }

            // Existing target metadata checks: symlink and hardlink safety
            if let Ok(meta) = fs::symlink_metadata(&target_path) {
                if meta.file_type().is_symlink() {
                    return Err(ExtractionError::SymlinkForbidden);
                }
                #[cfg(unix)]
                if meta.nlink() > 1 {
                    return Err(ExtractionError::HardlinkForbidden);
                }
            }

            // Stream extraction bounded by remaining quota
            let remaining_quota = max_bytes.saturating_sub(total_bytes);
            let mut limited_reader = (&mut entry).take(remaining_quota + 1);

            let mut out_file = File::create(&target_path)?;
            let bytes = io::copy(&mut limited_reader, &mut out_file)?;

            if bytes > remaining_quota {
                let _ = fs::remove_file(&target_path);
                return Err(ExtractionError::QuotaExceeded);
            }

            let canonical_target = target_path.canonicalize()?;
            if !canonical_target.starts_with(&dest_canonical) {
                let _ = fs::remove_file(&target_path);
                return Err(ExtractionError::ZipSlipDetected);
            }

            files_extracted += 1;
            total_bytes += bytes;
        }
    }

    Ok(ExtractionReport {
        files_extracted,
        total_bytes,
    })
}

/// Statistics about a completed skin packaging operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReport {
    pub files_packaged: usize,
    pub total_uncompressed_bytes: u64,
    pub package_size: u64,
}

/// Packs a Rainmeter skin directory into a `.rmskin` (or standard `.zip`) archive.
///
/// Walks `source_dir` recursively and writes all non-symlink regular files into `output_archive`.
/// Path separators in the archive are normalized to forward slashes.
pub fn pack_rmskin_package<P: AsRef<Path>, Q: AsRef<Path>>(
    source_dir: P,
    output_archive: Q,
) -> Result<PackageReport, ExtractionError> {
    let source_dir = source_dir.as_ref();
    let output_archive = output_archive.as_ref();

    if !source_dir.is_dir() {
        return Err(ExtractionError::IoError(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Source directory '{}' does not exist or is not a directory", source_dir.display()),
        )));
    }

    if let Some(parent) = output_archive.parent() {
        fs::create_dir_all(parent)?;
    }

    let file = File::create(output_archive)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    let mut files_packaged = 0;
    let mut total_uncompressed_bytes = 0;

    fn walk_dir_and_pack(
        root: &Path,
        current: &Path,
        zip: &mut zip::ZipWriter<File>,
        options: &zip::write::SimpleFileOptions,
        files_count: &mut usize,
        uncompressed_bytes: &mut u64,
    ) -> Result<(), ExtractionError> {
        let entries = fs::read_dir(current)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            let meta = fs::symlink_metadata(&path)?;

            if meta.file_type().is_symlink() {
                continue;
            }

            if meta.is_dir() {
                walk_dir_and_pack(root, &path, zip, options, files_count, uncompressed_bytes)?;
            } else if meta.is_file() {
                let rel_path = path.strip_prefix(root).map_err(|e| {
                    io::Error::new(io::ErrorKind::InvalidInput, e.to_string())
                })?;
                let rel_str = rel_path.to_string_lossy().replace('\\', "/");
                if rel_str.is_empty() {
                    continue;
                }

                zip.start_file(rel_str, *options)?;
                let mut f = File::open(&path)?;
                let mut buffer = Vec::new();
                let bytes = f.read_to_end(&mut buffer)?;
                use std::io::Write;
                zip.write_all(&buffer)?;

                *files_count += 1;
                *uncompressed_bytes += bytes as u64;
            }
        }
        Ok(())
    }

    walk_dir_and_pack(
        source_dir,
        source_dir,
        &mut zip,
        &options,
        &mut files_packaged,
        &mut total_uncompressed_bytes,
    )?;

    zip.finish()?;

    let package_size = fs::metadata(output_archive)?.len();

    Ok(PackageReport {
        files_packaged,
        total_uncompressed_bytes,
        package_size,
    })
}
