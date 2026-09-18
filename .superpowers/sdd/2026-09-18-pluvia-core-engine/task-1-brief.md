# Task 1 Brief: Rust Workspace Scaffolding & Charset Transcoder

## Overview
Initialize the Cargo workspace for Pluvia and implement the automatic charset transcoding module in `crates/pluvia-core/src/encoding.rs`.
The function `decode_ini_bytes(raw: &[u8]) -> Result<String, EncodingError>` must:
1. Detect UTF-16 LE BOM (`0xFF, 0xFE`) and decode via `encoding_rs::UTF_16LE`.
2. Detect UTF-16 BE BOM (`0xFE, 0xFF`) and decode via `encoding_rs::UTF_16BE`.
3. Detect UTF-8 BOM (`0xEF, 0xBB, 0xBF`) and strip it.
4. Try UTF-8 decoding.
5. If invalid UTF-8, fall back to Windows-1252 (`encoding_rs::WINDOWS_1252`).

## Target Files
- `Cargo.toml` (root workspace with members `crates/pluvia-core`, `crates/pluvia-daemon`, `crates/pluvia-cli`)
- `crates/pluvia-core/Cargo.toml`
- `crates/pluvia-core/src/lib.rs`
- `crates/pluvia-core/src/encoding.rs`
- `crates/pluvia-core/tests/test_encoding.rs`

## Instructions
1. Follow TDD: create failing unit test first in `tests/test_encoding.rs`.
2. Verify failure with `cargo test -p pluvia-core --test test_encoding`.
3. Implement `encoding.rs` and workspace configuration.
4. Verify tests pass with `cargo test -p pluvia-core --test test_encoding`.
5. Commit with message: `feat(core): setup workspace and implement auto-transcoding for UTF-16LE and Windows-1252`
6. Write brief completion report to `.superpowers/sdd/2026-09-18-pluvia-core-engine/task-1-report.md`.
