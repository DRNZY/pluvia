# Task 3 Report: Rainmeter .ini Lexer, Parser & Expression Engine

## Summary
Implemented the complete Rainmeter `.ini` parsing pipeline and mathematical expression evaluator in `pluvia-core`. These components provide full compatibility with Windows-authored Rainmeter skins, supporting case-insensitive section and key lookups, nested `#VarName#` expansion, built-in macro resolution (`#@#` -> `@Resources/`, `#CURRENTPATH#`, etc.), recursive `@Include` directives with virtual filesystem (`VfsResolver`) resolution and circular include detection, style inheritance (`MeterStyle`), and an arithmetic Pratt parser evaluating formulas and mathematical functions.

## Implemented Components

1. **Variables Engine (`crates/pluvia-core/src/variables.rs`)**:
   - `VariableMap`:
     - Internal `HashMap<String, String>` normalizing keys to lowercase for case-insensitive lookup.
     - Built-in macro support: initializes `#@#` (`"@"`) to `"@Resources/"`.
     - `with_skin_dir(path)` sets `#CURRENTPATH#` and `#SKINSPATH#` with trailing slash normalization.
     - `set`, `get`, `get_num`, `contains_key`, `remove`, `len`, `is_empty`, and `iter`.
     - `expand(input: &str) -> String`: Multi-pass `#VarName#` macro expansion (up to 16 passes) resolving chained variables while preserving unmatched hashes (e.g. hex colors `#1E293B`).

2. **Mathematical Formula Evaluator (`crates/pluvia-core/src/formulas.rs`)**:
   - `VariableLookup` trait for flexible variable resolution across `VariableMap`, `HashMap<String, f64>`, `HashMap<String, String>`, and references.
   - Pratt expression parser supporting:
     - Binary operators: `+`, `-`, `*`, `/`, `%`, `^`, `**` (with right-associative exponentiation).
     - Unary operators: `+`, `-`, `!`.
     - Comparison and logical operators: `==`, `=`, `!=`, `<>`, `<`, `<=`, `>`, `>=`, `&&`, `&`, `||`, `|`.
     - Ternary conditional expressions: `condition ? true_expr : false_expr`.
     - Variable resolution for both `#VarName#` syntax and bare identifiers (`w`, `scale`).
     - Functions: `Round` (with optional decimal precision), `Trunc`, `Abs`, `Min`, `Max`, `Clamp`, `Sin`, `Cos`, `Tan`, `Asin`, `Acos`, `Atan`, `Atan2`, `Floor`, `Ceil`, `Sqrt`, `Log`/`Ln`, `Log10`, `Exp`, `Rad`, `Deg`, `Sgn`/`Sign`, `Frac`.
     - Safe error handling: explicit `FormulaError::DivisionByZero`, `FormulaError::SyntaxError`, `FormulaError::UnknownVariable`, `FormulaError::UnknownFunction`, `FormulaError::InvalidArguments`, and `FormulaError::EmptyExpression`.

3. **Rainmeter INI Parser (`crates/pluvia-core/src/ini.rs`)**:
   - `parse_skin_ini<P: AsRef<Path>>(content: &str, skin_dir: P) -> Result<SkinConfig, ParseError>`
   - `parse_skin_file<P: AsRef<Path>>(path: P) -> Result<SkinConfig, ParseError>`:
     - Automatically transcodes files using `decode_ini_bytes` (UTF-16 LE, Windows-1252, UTF-8 BOM).
   - Recursive `@Include` handling:
     - Case-insensitive directive matching (`@Include`, `@Include1`, `@IncludeVariables`, etc.).
     - Path resolution using `VfsResolver` across current file directory, skin root directory, parent suite directory, and absolute paths.
     - Circular include protection via active call-stack cycle detection (`ParseError::CircularInclude`) that permits legitimate diamond dependencies.
   - Section and option extraction:
     - `[Rainmeter]`: parses `Update`, `AccurateText`, `DynamicWindowSize`.
     - `[Variables]`: populates `VariableMap` and resolves chained variables.
     - Measures (`Measure=...`): typed `MeasureConfig` with `measure_type`, `plugin`, `format`, `formula`, `update_divider`, `disabled`, `dynamic_variables`, and expanded properties.
     - Meters (`Meter=...`): typed `MeterConfig` with `meter_type`, `measure_name`, `measure_names` (deterministically indexed by numeric suffix), `font_face`, `font_size` (formula evaluated), `font_color`, `text` (quotes stripped), `anti_alias`, `x`, `y`, `w`, `h`, `solid_color`, `hidden`, `dynamic_variables`, and expanded properties.
     - Style inheritance (`apply_meter_styles`): merges properties from referenced `MeterStyle` sections using Rainmeter rightmost precedence while respecting meter explicit overrides.
     - Preserves original casing for names while allowing case-insensitive map lookups.
     - Preserves section execution/rendering order in `measure_order` and `meter_order`.

4. **Module Exports (`crates/pluvia-core/src/lib.rs`)**:
   - Exposed `pub mod formulas;`, `pub mod ini;`, and `pub mod variables;`.

## Tests & Verification

- **TDD RED Phase**:
  Initial test run of `crates/pluvia-core/tests/test_parser.rs` failed as expected with exit code 101 due to missing `ini`, `variables`, and `formulas` modules.
- **TDD GREEN Phase**:
  Implemented components and achieved clean pass on `cargo test -p pluvia-core --test test_parser` (14 tests passed in 0.00s):
  1. `test_parse_mond_clock_ini`: Mond Clock sample parsing, update rate, variables, meter properties, formula evaluation.
  2. `test_formula_math_evaluation`: Formula evaluation with `HashMap<String, f64>` and `#Var#` substitution.
  3. `test_case_insensitive_key_and_section_retrieval`: Case-insensitivity across sections, keys, measures, and meters.
  4. `test_formula_advanced_math_functions`: Trunc, Abs, Min, Max, Clamp, Sin, Cos, `**`, `^`, `%`, nested function calls.
  5. `test_include_directive_expansion_with_vfs`: `@Include` directive with `#@#` expansion and VFS path resolution.
  6. `test_builtin_resource_macro_resolution`: `#@#` expansion to `@Resources/` across forward and Windows backslashes.
  7. `test_division_by_zero_error`: Returns `FormulaError::DivisionByZero`.
  8. `test_nested_include_and_style_inheritance`: Multi-level nested includes with `MeterStyle` composition (`StyleBase | StyleAccent`).
  9. `test_circular_include_detection`: Detection and rejection of active circular includes (`a.inc <-> b.inc`).
  10. `test_diamond_dependency_includes`: Diamond dependencies (`A -> B -> SharedVars` and `A -> C -> SharedVars`) parse cleanly without false-positive circular errors.
  11. `test_meter_style_override_precedence`: Verifies `MeterStyle=Base | Override` allows rightmost `Override` to supersede `Base`, with explicit meter options taking top precedence.
  12. `test_meter_multi_measure_ordering`: Verifies `MeasureName`, `MeasureName2`, `MeasureName3` are preserved in exact numerical index order regardless of raw INI option ordering.
  13. `test_formula_ternary_and_comparisons`: Ternary operators `? :` and comparisons (`==`, `!=`, `>`, `>=`).
  14. `test_comments_and_empty_lines`: Proper comment skipping (`;`) and preservation of semicolons within option values.
- **Workspace-wide Tests**:
  `cargo test --all` ran and passed all 32 tests across `pluvia-core`, `pluvia-daemon`, and `pluvia-cli` with zero errors.
- **Compilation Check**:
  `cargo check --all --tests` succeeded with 0 compiler warnings.

## Fix Round 1 Notes
- **Diamond Dependency Resolution**: Replaced permanent `visited_files` set with active call stack `include_stack: Vec<PathBuf>`. Pushes canonical path on entering `parse_content_recursive` and pops upon exit. True cycles (`A -> B -> A`) are rejected, while diamond dependencies (`A -> B -> Common` and `A -> C -> Common`) succeed cleanly.
- **MeterStyle Rightmost Precedence**: Replaced left-to-right style application with reverse-order iteration (`styles_str.split('|').map(str::trim).rev()`). Rightmost styles insert missing keys first; earlier styles cannot overwrite later styles; explicit meter options remain inviolate.
- **Deterministic Multi-Measure Ordering**: Implemented index parser for `MeasureName` (index 1) and `MeasureName<N>` (index N), sorting entries numerically before populating `meter.measure_names` and `meter.measure_name`.

## Git Commits
- `7e49ee5`: `feat(core): implement Rainmeter INI parser and formula evaluator`
- `0fb4ea3`: `fix(core): resolve diamond include false-positive, fix MeterStyle precedence, and ensure deterministic measure bindings`
