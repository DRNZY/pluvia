# Task 1 Report: Rust Workspace Scaffolding & Charset Transcoder

## Summary
Successfully scaffolded the Pluvia multi-crate Cargo workspace and implemented the charset auto-transcoding engine in `crates/pluvia-core/src/encoding.rs`. The transcode function transparently handles legacy and modern Rainmeter `.ini` file encodings.

## Implemented Components
1. **Workspace Architecture**:
   - `Cargo.toml`: Root workspace configuration declaring members `crates/pluvia-core`, `crates/pluvia-daemon`, and `crates/pluvia-cli`, with shared workspace dependencies (`serde`, `tokio`, `thiserror`, `anyhow`, `tracing`).
   - `crates/pluvia-daemon/`: Minimal executable package stub.
   - `crates/pluvia-cli/`: Minimal executable package stub.
   - `crates/pluvia-core/`: Core library package including `encoding_rs` and `thiserror`.

2. **Charset Transcoder (`crates/pluvia-core/src/encoding.rs`)**:
   - `decode_ini_bytes(raw: &[u8]) -> Result<String, EncodingError>`:
     - Detects UTF-16 LE BOM (`[0xFF, 0xFE]`) and decodes using `encoding_rs::UTF_16LE`. If malformed, immediately returns `Err(EncodingError::DecodingFailed)`.
     - Detects UTF-16 BE BOM (`[0xFE, 0xFF]`) and decodes using `encoding_rs::UTF_16BE`. If malformed, immediately returns `Err(EncodingError::DecodingFailed)`.
     - Detects UTF-8 BOM (`[0xEF, 0xBB, 0xBF]`) and strips it.
     - Performs zero-copy UTF-8 validation via `std::str::from_utf8`.
     - Gracefully falls back to Windows-1252 / CP1252 via `encoding_rs::WINDOWS_1252`.
     - Handles empty byte buffers returning an empty string.

3. **Tests (`crates/pluvia-core/tests/test_encoding.rs`)**:
   - `test_decode_utf8_with_and_without_bom`: Verified UTF-8 string decoding with and without BOM.
   - `test_decode_utf16_le_with_bom`: Verified UTF-16 LE BOM decoding.
   - `test_decode_utf16_be_with_bom`: Verified UTF-16 BE BOM decoding.
   - `test_decode_windows_1252_ansi`: Verified smart-quote / ANSI fallback decoding strictly preserving curly quotes (`“` and `”`).
   - `test_decode_empty_bytes`: Verified empty buffer decoding.
   - `test_decode_malformed_utf16_le`: Verified negative case returns `Err(EncodingError::DecodingFailed)` for odd-length and surrogate errors.
   - `test_decode_malformed_utf16_be`: Verified negative case returns `Err(EncodingError::DecodingFailed)` for odd-length and surrogate errors.

## Verification
- **TDD Workflow**: Test initially executed and verified to fail prior to module implementation.
- **Cargo Test**: `cargo test -p pluvia-core --test test_encoding` passed all 7 tests.
- **Workspace Verification**: `cargo test --workspace` and `cargo check --workspace` all passed cleanly.

## Fix Round 1 Notes
- **UTF-16 BOM Error Handling**: Fixed fall-through bug where malformed UTF-16 BOM inputs fell through to Windows-1252; now immediately returns `Err(EncodingError::DecodingFailed)`.
- **Negative Unit Tests**: Added unit tests for malformed UTF-16 LE and BE (odd byte length, unpaired surrogates).
- **Assertion Tightened**: Strengthened `test_decode_windows_1252_ansi` to assert exact decoded string `"Text=“Hi”"`.
- **Dependency Cleanup**: Removed unused `anyhow.workspace = true` in `crates/pluvia-core/Cargo.toml`.

## Git Commits
- `eb9a601`: `feat(core): setup workspace and implement auto-transcoding for UTF-16LE and Windows-1252`
- `4a0e1df`: `fix(core): return DecodingFailed on malformed UTF-16 BOM and add negative tests`
