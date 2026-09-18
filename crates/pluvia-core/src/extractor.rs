use std::fs::{self, File};
use std::io;
use std::path::{Component, Path};
use thiserror::Error;

/// Errors that can occur during `.rmskin` package extraction.
#[derive(Error, Debug)]
pub enum ExtractionError {
    #[error("Zip-Slip directory traversal detected")]
    ZipSlipDetected,
    #[error("Symlink extraction is strictly forbidden")]
    SymlinkForbidden,
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
/// - Absolute anti-Zip-Slip enforcement (rejects parent directory components, root components,
///   and path escapes via canonical prefix verification).
/// - Strict symlink immunity (any symlink entry detected via Unix mode attributes is rejected).
/// - Rejects pre-existing symlinks in destination paths that could allow symlink-following escapes.
pub fn extract_rmskin_package<P: AsRef<Path>, Q: AsRef<Path>>(
    archive_path: P,
    dest_root: Q,
) -> Result<ExtractionReport, ExtractionError> {
    let dest_root = dest_root.as_ref();
    fs::create_dir_all(dest_root)?;
    let dest_canonical = dest_root.canonicalize()?;

    let file = File::open(archive_path)?;
    let mut zip = zip::ZipArchive::new(file)?;

    let mut files_extracted = 0;
    let mut total_bytes = 0;

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

        // 2. Component inspection on raw entry name
        let raw_name = entry.name();
        let raw_path = Path::new(raw_name);
        if raw_path.components().any(|c| {
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

        let target_path = dest_canonical.join(&enclosed);

        // 4. Directory entry extraction
        if entry.is_dir() {
            fs::create_dir_all(&target_path)?;
            let canonical = target_path.canonicalize()?;
            if !canonical.starts_with(&dest_canonical) {
                return Err(ExtractionError::ZipSlipDetected);
            }
        } else {
            // 5. File entry extraction
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent)?;
                let canonical_parent = parent.canonicalize()?;
                if !canonical_parent.starts_with(&dest_canonical) {
                    return Err(ExtractionError::ZipSlipDetected);
                }
            }

            // Reject if target path is already an existing symlink
            if let Ok(meta) = fs::symlink_metadata(&target_path) {
                if meta.file_type().is_symlink() {
                    return Err(ExtractionError::SymlinkForbidden);
                }
            }

            let mut out_file = File::create(&target_path)?;
            let bytes = io::copy(&mut entry, &mut out_file)?;

            let canonical_target = target_path.canonicalize()?;
            if !canonical_target.starts_with(&dest_canonical) {
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
