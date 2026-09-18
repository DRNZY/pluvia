# Task 2 Report: Case-Insensitive VFS Path Resolver & Anti-Zip-Slip Extractor

## Summary
Successfully implemented the virtual file system (`VfsResolver`) and secure package extraction engine (`extract_rmskin_package`) in `pluvia-core`. These components provide seamless compatibility with Windows-authored Rainmeter skins while strictly isolating and defending against malicious directory traversal and symlink vulnerabilities.

## Implemented Components
1. **Case-Insensitive VFS Path Resolver (`crates/pluvia-core/src/vfs.rs`)**:
   - `VfsResolver::normalize_rel_path(rel_path: &str) -> String`:
     - Strips Windows drive letters (e.g., `C:\...` or `c:/...`).
     - Translates all Windows backslashes `\` to forward slashes `/`.
     - Trims leading `/` to guarantee relative resolution.
   - `VfsResolver::resolve<P: AsRef<Path>>(&self, base: P, raw_rel: &str) -> Option<PathBuf>`:
     - Normalizes incoming path and checks an in-memory `RwLock<HashMap<String, PathBuf>>` cache for $O(1)$ hits.
     - Performs segment-by-segment traversal; handles `..` and `.`.
     - Direct existence check fast-path followed by case-insensitive directory scanning (`entry_name.eq_ignore_ascii_case(segment)`).
     - Successfully resolved paths are persisted into the cache.
   - Cache inspectability helpers: `is_cached`, `cache_len`, and `clear_cache`.

2. **Anti-Zip-Slip Package Extractor (`crates/pluvia-core/src/extractor.rs`)**:
   - `extract_rmskin_package<P: AsRef<Path>, Q: AsRef<Path>>(archive_path: P, dest_root: Q) -> Result<ExtractionReport, ExtractionError>`:
     - Canonicalizes destination root prior to extraction.
     - **Symlink Prohibition**: Rejects any entry where `entry.is_symlink()` is true or Unix mode bits match `0o120000` with `ExtractionError::SymlinkForbidden`.
     - **Component Inspection**: Rejects raw entry names or `enclosed_name()` containing `Component::ParentDir`, `Component::Prefix`, or `Component::RootDir` with `ExtractionError::ZipSlipDetected`.
     - **Canonical Prefix Verification**: Confirms directory and file paths are strictly prefixed by `dest_canonical`.
     - **Pre-existing Symlink Immunity**: Rejects target paths that exist as symlinks before writing to prevent symlink-following escapes.
     - Returns `ExtractionReport` with `files_extracted` count and `total_bytes` written.

3. **Crate Configuration & Module Exports**:
   - `crates/pluvia-core/Cargo.toml`: Added `zip = "2.2"` to dependencies and `tempfile = "3.12"` to dev-dependencies.
   - `crates/pluvia-core/src/lib.rs`: Exposed `pub mod vfs;` and `pub mod extractor;`.

4. **Tests (`crates/pluvia-core/tests/test_vfs_and_extractor.rs`)**:
   - `test_vfs_case_insensitive_and_backslash_resolution`: Validates resolution of `@resources\fonts\anurati.otf` against `@Resources/Fonts/Anurati.otf` on disk, as well as Windows drive letter stripping (`C:\...`).
   - `test_vfs_cached_lookup_verification`: Confirms that lookups populate the cache and return the cached path without disk access even if the target file is deleted.
   - `test_vfs_case_folded_cache_hit`: Verifies case-folded cache keys (`clock.ini` vs `Clock.ini` share single cache entry).
   - `test_vfs_base_jailing`: Verifies `..` traversal cannot escape `base`.
   - `test_anti_zip_slip_path_traversal`: Validates that directory traversal attempts (`../etc/passwd`) return `ExtractionError::ZipSlipDetected`.
   - `test_anti_zip_slip_windows_backslash_traversal`: Validates that Windows backslash traversal attempts (`..\..\evil.txt`) return `ExtractionError::ZipSlipDetected`.
   - `test_strict_symlink_rejection`: Validates that symlink zip entries return `ExtractionError::SymlinkForbidden`.
   - `test_directory_symlink_rejection`: Validates that pre-existing directory symlinks on destination paths trigger `ExtractionError::SymlinkForbidden`.
   - `test_hardlink_target_forbidden`: Validates that writing over files with `nlink > 1` triggers `ExtractionError::HardlinkForbidden`.
   - `test_quota_exceeded_zip_bomb`: Validates that extractions exceeding the byte quota abort and return `ExtractionError::QuotaExceeded`.
   - `test_valid_zip_extraction_and_report`: Confirms clean extraction of multi-file package and verified `ExtractionReport` counts and contents.

## Verification
- **TDD Failure Phase (RED)**: Initial test run confirmed failure due to missing modules `vfs` and `extractor` with exit code 101.
- **TDD Success Phase (GREEN)**: `cargo test -p pluvia-core --test test_vfs_and_extractor` passed all 11 unit tests in 0.00s.
- **Workspace-wide Tests**: `cargo test --all` passed all 18 tests (7 encoding + 11 vfs/extractor) with zero compiler warnings.

## Fix Round 1 Notes
- **Hardlink Protection**: Added `meta.nlink() > 1` check on Unix targets, returning `ExtractionError::HardlinkForbidden` to prevent mutating shared inodes.
- **Directory Symlink Immunity**: Added component-wise and target symlink checks stripping trailing slashes to prevent POSIX `lstat` auto-dereferencing of directory symlinks, returning `ExtractionError::SymlinkForbidden`.
- **Zip Bomb Decompression Quota**: Introduced `MAX_EXTRACTION_BYTES` (500 MB) safety cap and `extract_rmskin_package_with_limit`, returning `ExtractionError::QuotaExceeded` and rolling back partial writes on breach.
- **Windows Backslash Traversal Defense**: Normalized backslashes in raw zip entry names prior to inspecting path components for `Component::ParentDir`.
- **Case-Folded VFS Cache Keys**: Formatted cache keys using `normalized.to_ascii_lowercase()` so case variants (`clock.ini` / `Clock.ini`) share identical cache entries.
- **VFS Base Jailing**: Prevented `..` popping above `base` (`if current != base { current.pop(); }`) and enforced `current.starts_with(base)`.
- **Cargo.toml Cleanup**: Moved `tempfile = "3.12"` to `[dev-dependencies]` in `crates/pluvia-core/Cargo.toml`.

## Git Commits
- `5820dde`: `feat(core): implement case-insensitive VFS and anti-zip-slip package extractor`
- `df4c094`: `fix(core): harden extractor against hardlinks, directory symlinks, and zip bombs; case-fold VFS cache`
