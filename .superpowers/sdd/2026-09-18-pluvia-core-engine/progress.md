# SDD ledger — plan: /home/darnell/Projects/pluvia/docs/superpowers/plans/2026-09-18-pluvia-core-engine.md

## Pre-Flight Plan Scan

| Task Pair / Task | Produces / Consumes | Scan Result | Ruling |
| :--- | :--- | :--- | :--- |
| Task 1 -> Task 2 | `pluvia-core` crate scaffolding | Consistent | Clean |
| Task 1 -> Task 3 | `decode_ini_bytes` consumed by `parse_skin_ini` | Consistent | Clean |
| Task 2 -> Task 3 | `VfsResolver` consumed by parser for `@Resources` | Consistent | Clean |
| Task 3 -> Task 4 | `SkinConfig` consumed by telemetry runner | Consistent | Clean |
| Task 4 -> Task 6 | `MeasureValue` consumed by meter renderer | Consistent | Clean |
| Task 5 -> Task 6 | `AudioLevelPlugin` & `ActionTimer` feeds Cairo | Consistent | Clean |
| Task 6 -> Task 7 | `AlphaHitMask` passed to layer-shell/X11 | Consistent | Clean |
| Task 7 -> Task 8 | Desktop surfaces driven by JSON-RPC | Consistent | Clean |
| Task 8 -> Task 9 | Full pipeline verified via Mond Clock E2E | Consistent | Clean |

## Tasks

- [x] Task 1: Rust Workspace Scaffolding & Charset Transcoder (commit: 4a0e1df)
- [x] Task 2: Case-Insensitive VFS Path Resolver & Anti-Zip-Slip Extractor (commit: df4c094)
- [x] Task 3: Rainmeter .ini Lexer, Parser & Expression Engine (commit: 0fb4ea3)
- [x] Task 4: Linux Telemetry & Measure Engine (commit: fe1bc69)
- [x] Task 5: Native C++ DLL Plugin Emulators (commit: a343ecc)
- [x] Task 6: PangoCairo 2D Meter Renderer & Alpha Hit-Test Masks (commit: 7ea7a59)
- [x] Task 7: Display Layer & Window Backend (commit: 7d64dc3)
- [x] Task 8: UNIX Domain Socket JSON-RPC 2.0 Server & CLI (commit: b647e79)
- [x] Task 9: Out-of-the-Box Mond Clock Integration & End-to-End Test (commit: c6d4c4c)
