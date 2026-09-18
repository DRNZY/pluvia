# Task 2 Brief: Case-Insensitive VFS Path Resolver & Anti-Zip-Slip Extractor

## Overview
Implement two core modules in `pluvia-core`:
1. `crates/pluvia-core/src/vfs.rs`: Case-insensitive path resolver and Windows path normalizer. Translates `\` to `/`, strips drive prefixes, and performs case-insensitive directory lookups with an in-memory cache.
2. `crates/pluvia-core/src/extractor.rs`: Secure `.rmskin` package extractor. Enforces strict anti-Zip-Slip protection (component inspection, canonical prefix verification) and explicitly forbids symlink entries.

## Target Files
- `crates/pluvia-core/Cargo.toml` (add `zip = "2.2"`, `tempfile = "3.12"`)
- `crates/pluvia-core/src/vfs.rs`
- `crates/pluvia-core/src/extractor.rs`
- `crates/pluvia-core/src/lib.rs` (expose `pub mod vfs; pub mod extractor;`)
- `crates/pluvia-core/tests/test_vfs_and_extractor.rs`

## Instructions
1. Follow TDD: create failing unit tests in `crates/pluvia-core/tests/test_vfs_and_extractor.rs` covering:
   - Case-insensitive path resolution and Windows `\` translation.
   - Cached lookup verification.
   - Zip-Slip path traversal attempt (`../etc/passwd`) returning `ExtractionError::ZipSlipDetected`.
   - Symlink entry attempt returning `ExtractionError::SymlinkForbidden`.
   - Valid zip extraction producing correct `ExtractionReport`.
2. Verify failure with `cargo test -p pluvia-core --test test_vfs_and_extractor`.
3. Implement `vfs.rs` and `extractor.rs`.
4. Verify all tests pass with `cargo test -p pluvia-core --test test_vfs_and_extractor`.
5. Commit with message: `feat(core): implement case-insensitive VFS and anti-zip-slip package extractor`
6. Write full completion report to `.superpowers/sdd/2026-09-18-pluvia-core-engine/task-2-report.md`.
7. Return status under 15 lines.
