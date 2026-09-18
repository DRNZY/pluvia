# Task 3 Brief: Rainmeter .ini Lexer, Parser & Expression Engine

## Overview
Implement the complete Rainmeter `.ini` parsing pipeline and mathematical formula evaluator in `pluvia-core`:
1. `crates/pluvia-core/src/ini.rs`: Robust INI lexer/parser tailored to Rainmeter. Handles case-insensitive section and key names, comment lines (`;`), includes (`@Include`), `#@#` expansion to `@Resources/`, section ordering, and categorizing into `[Rainmeter]`, `[Variables]`, Measures (`Measure=...`), and Meters (`Meter=...`).
2. `crates/pluvia-core/src/variables.rs`: Variable storage, case-insensitive lookup, expansion of `#VarName#`, built-in variables (`#@#`, `#CURRENTPATH#`), and dynamic variable resolution.
3. `crates/pluvia-core/src/formulas.rs`: Mathematical expression evaluator supporting arithmetic (`+`, `-`, `*`, `/`, `%`), parentheses, variable substitutions, and functions (`Round`, `Trunc`, `Abs`, `Min`, `Max`, `Clamp`, `Sin`, `Cos`).

## Target Files
- `crates/pluvia-core/src/ini.rs`
- `crates/pluvia-core/src/variables.rs`
- `crates/pluvia-core/src/formulas.rs`
- `crates/pluvia-core/src/lib.rs` (expose `pub mod ini; pub mod variables; pub mod formulas;`)
- `crates/pluvia-core/tests/test_parser.rs`

## Instructions
1. Follow TDD: create failing unit tests in `crates/pluvia-core/tests/test_parser.rs` covering:
   - Parsing of typical skins (like Mond Clock with `[Rainmeter]`, `[Variables]`, `[MeasureTime]`, `[MeterTime]`).
   - Case-insensitive key/section retrieval (`meter.font_face` matching `FontFace=...`).
   - Formula evaluations with variables (e.g. `(#W# * #Scale#) + 10` and `Round(14.7)`).
   - `@Include` directive expansion with VFS resolution.
   - Built-in `#@#` macro resolution to `@Resources/`.
2. Verify failure with `cargo test -p pluvia-core --test test_parser`.
3. Implement `ini.rs`, `variables.rs`, `formulas.rs`.
4. Verify all tests pass with `cargo test -p pluvia-core --test test_parser`.
5. Commit with message: `feat(core): implement Rainmeter INI parser and formula evaluator`
6. Write completion report to `.superpowers/sdd/2026-09-18-pluvia-core-engine/task-3-report.md`.
7. Return status under 15 lines.
