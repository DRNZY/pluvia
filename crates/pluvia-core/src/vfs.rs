use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

/// Virtual File System (VFS) resolver for Rainmeter paths.
///
/// Rainmeter configurations were written for Windows NTFS filesystems, which are case-insensitive
/// and use `\` path separators. On Linux, filesystems are case-sensitive and use `/`.
/// `VfsResolver` normalizes paths and resolves case mismatches dynamically, caching hits for $O(1)$
/// subsequent lookups using case-folded cache keys.
pub struct VfsResolver {
    cache: RwLock<HashMap<String, PathBuf>>,
}

impl VfsResolver {
    /// Creates a new empty `VfsResolver`.
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// Normalizes a relative or Windows path:
    /// - Strips Windows drive letters (e.g., `C:\...` or `c:/...`)
    /// - Replaces Windows backslashes `\` with forward slashes `/`
    /// - Strips leading slashes to ensure a relative path
    pub fn normalize_rel_path(rel_path: &str) -> String {
        let trimmed = rel_path.trim();

        // Strip drive letter if present: e.g., "C:" or "c:"
        let without_drive = if trimmed.len() >= 2
            && trimmed.as_bytes()[1] == b':'
            && trimmed.as_bytes()[0].is_ascii_alphabetic()
        {
            &trimmed[2..]
        } else {
            trimmed
        };

        without_drive.replace('\\', "/").trim_start_matches('/').to_string()
    }

    /// Resolves a path relative to `base`, performing case-insensitive matching across each path
    /// component if direct lookup fails. Successful resolutions are cached under a case-folded key.
    /// Traversal is strictly jailed to `base` (cannot escape above `base` via `..`).
    pub fn resolve<P: AsRef<Path>>(&self, base: P, raw_rel: &str) -> Option<PathBuf> {
        let base = base.as_ref();
        let normalized = Self::normalize_rel_path(raw_rel);
        let cache_key = format!("{}::{}", base.display(), normalized.to_ascii_lowercase());

        // Check cache first (case-folded key ensures `clock.ini` and `Clock.ini` hit same entry)
        if let Ok(read_guard) = self.cache.read() {
            if let Some(cached) = read_guard.get(&cache_key) {
                return Some(cached.clone());
            }
        }

        let mut current = base.to_path_buf();
        for segment in normalized.split('/') {
            if segment.is_empty() || segment == "." {
                continue;
            }
            if segment == ".." {
                // Enforce base jailing: do not pop above base
                if current != base {
                    current.pop();
                }
                continue;
            }

            let direct = current.join(segment);
            if direct.exists() {
                current = direct;
                continue;
            }

            // Case-insensitive directory scan
            let mut found = false;
            if let Ok(entries) = fs::read_dir(&current) {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    if name.to_string_lossy().eq_ignore_ascii_case(segment) {
                        current = entry.path();
                        found = true;
                        break;
                    }
                }
            }

            if !found {
                return None;
            }
        }

        if current.exists() && current.starts_with(base) {
            if let Ok(mut write_guard) = self.cache.write() {
                write_guard.insert(cache_key, current.clone());
            }
            Some(current)
        } else {
            None
        }
    }

    /// Checks whether a given path resolution is currently cached (using case-folded lookup).
    pub fn is_cached<P: AsRef<Path>>(&self, base: P, raw_rel: &str) -> bool {
        let base = base.as_ref();
        let normalized = Self::normalize_rel_path(raw_rel);
        let cache_key = format!("{}::{}", base.display(), normalized.to_ascii_lowercase());
        if let Ok(guard) = self.cache.read() {
            guard.contains_key(&cache_key)
        } else {
            false
        }
    }

    /// Returns the number of cached resolutions.
    pub fn cache_len(&self) -> usize {
        if let Ok(guard) = self.cache.read() {
            guard.len()
        } else {
            0
        }
    }

    /// Clears all cached resolutions.
    pub fn clear_cache(&self) {
        if let Ok(mut guard) = self.cache.write() {
            guard.clear();
        }
    }
}

impl Default for VfsResolver {
    fn default() -> Self {
        Self::new()
    }
}
