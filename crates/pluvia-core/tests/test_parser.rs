use pluvia_core::ini::parse_skin_ini;
use pluvia_core::variables::VariableMap;
use pluvia_core::formulas::{eval_formula, FormulaError};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use tempfile::tempdir;

#[test]
fn test_parse_mond_clock_ini() {
    let sample_ini = r#"
[Rainmeter]
Update=1000
AccurateText=1
DynamicWindowSize=1

[Variables]
FontName=Anurati
Color1=226,232,240
Scale=1.2

[MeasureTime]
Measure=Time
Format=%H:%M

[MeterTime]
Meter=String
MeasureName=MeasureTime
FontFace=#FontName#
FontColor=#Color1#
FontSize=(16 * #Scale#)
Text="Now: %1"
AntiAlias=1
"#;

    let config = parse_skin_ini(sample_ini, Path::new("/dummy")).unwrap();
    assert_eq!(config.update_rate_ms, 1000);
    assert_eq!(config.variables.get("fontname").unwrap(), "Anurati");

    let meter = config.meters.get("metertime").unwrap();
    assert_eq!(meter.font_face.as_deref(), Some("Anurati"));
    assert_eq!(meter.font_size, Some(19.2));
}

#[test]
fn test_formula_math_evaluation() {
    let mut vars = HashMap::new();
    vars.insert("w".to_string(), 100.0);
    vars.insert("scale".to_string(), 1.5);

    assert_eq!(eval_formula("(#W# * #Scale#) + 10", &vars).unwrap(), 160.0);
    assert_eq!(eval_formula("Round(14.7)", &vars).unwrap(), 15.0);
}

#[test]
fn test_case_insensitive_key_and_section_retrieval() {
    let ini = r#"
[rAiNmEtEr]
uPdAtE=2000

[vArIaBlEs]
mYfOnT=Segoe UI

[mEaSuReCpU]
mEaSuRe=Cpu

[mEtErClOcK]
mEtEr=String
fOnTfAcE=#mYfOnT#
fOnTsIzE=24
tExT="Hello"
"#;

    let config = parse_skin_ini(ini, Path::new("/dummy")).unwrap();
    assert_eq!(config.update_rate_ms, 2000);
    assert_eq!(config.variables.get("myfont").unwrap(), "Segoe UI");
    assert_eq!(config.variables.get("MYFONT").unwrap(), "Segoe UI");

    let measure = config.measures.get("measurecpu").unwrap();
    assert_eq!(measure.measure_type, "Cpu");
    assert_eq!(measure.get("measure").unwrap(), "Cpu");

    let meter = config.meters.get("meterclock").unwrap();
    assert_eq!(meter.font_face.as_deref(), Some("Segoe UI"));
    assert_eq!(meter.font_size, Some(24.0));
    assert_eq!(meter.get("fontface").unwrap(), "Segoe UI");
    assert_eq!(meter.get("FONTFACE").unwrap(), "Segoe UI");
}

#[test]
fn test_formula_advanced_math_functions() {
    let vars = VariableMap::new();

    assert_eq!(eval_formula("Trunc(14.7)", &vars).unwrap(), 14.0);
    assert_eq!(eval_formula("Abs(-42.5)", &vars).unwrap(), 42.5);
    assert_eq!(eval_formula("Min(10, 5, 20)", &vars).unwrap(), 5.0);
    assert_eq!(eval_formula("Max(10, 5, 20)", &vars).unwrap(), 20.0);
    assert_eq!(eval_formula("Clamp(15, 0, 10)", &vars).unwrap(), 10.0);
    assert_eq!(eval_formula("Clamp(-5, 0, 10)", &vars).unwrap(), 0.0);
    assert_eq!(eval_formula("Clamp(5, 0, 10)", &vars).unwrap(), 5.0);
    assert_eq!(eval_formula("Sin(0)", &vars).unwrap(), 0.0);
    assert_eq!(eval_formula("Cos(0)", &vars).unwrap(), 1.0);
    assert_eq!(eval_formula("2 ** 3", &vars).unwrap(), 8.0);
    assert_eq!(eval_formula("2 ^ 4", &vars).unwrap(), 16.0);
    assert_eq!(eval_formula("17 % 5", &vars).unwrap(), 2.0);
    assert_eq!(eval_formula("Round(Min(15.6, 20.1))", &vars).unwrap(), 16.0);
}

#[test]
fn test_include_directive_expansion_with_vfs() {
    let tmp = tempdir().unwrap();
    let res_dir = tmp.path().join("@Resources");
    fs::create_dir_all(&res_dir).unwrap();

    let inc_file = res_dir.join("Variables.inc");
    let mut f = File::create(&inc_file).unwrap();
    f.write_all(b"[Variables]\nIncludedFont=Hack\nScale=2.0\n").unwrap();

    let main_ini = r#"
[Rainmeter]
Update=500

[Variables]
@Include=#@#Variables.inc
Theme=Dark

[MeterSample]
Meter=String
FontFace=#IncludedFont#
FontSize=(10 * #Scale#)
"#;

    let config = parse_skin_ini(main_ini, tmp.path()).unwrap();
    assert_eq!(config.variables.get("includedfont").unwrap(), "Hack");
    assert_eq!(config.variables.get("scale").unwrap(), "2.0");
    assert_eq!(config.variables.get("theme").unwrap(), "Dark");

    let meter = config.meters.get("metersample").unwrap();
    assert_eq!(meter.font_face.as_deref(), Some("Hack"));
    assert_eq!(meter.font_size, Some(20.0));
}

#[test]
fn test_builtin_resource_macro_resolution() {
    let vars = VariableMap::new();
    let expanded = vars.expand("#@#Fonts/Anurati.otf");
    assert_eq!(expanded, "@Resources/Fonts/Anurati.otf");

    let expanded_backslash = vars.expand("#@#Fonts\\Anurati.otf");
    assert_eq!(expanded_backslash, "@Resources/Fonts\\Anurati.otf");
}

#[test]
fn test_division_by_zero_error() {
    let vars = VariableMap::new();
    let res = eval_formula("10 / 0", &vars);
    assert!(matches!(res, Err(FormulaError::DivisionByZero)));
}

#[test]
fn test_nested_include_and_style_inheritance() {
    let tmp = tempdir().unwrap();
    let res_dir = tmp.path().join("@Resources");
    fs::create_dir_all(&res_dir).unwrap();

    let style_file = res_dir.join("Styles.inc");
    let mut f1 = File::create(&style_file).unwrap();
    f1.write_all(b"[StyleBase]\nFontFace=Roboto\nFontSize=14\nAntiAlias=1\n\n[StyleAccent]\nFontColor=255,0,0\n").unwrap();

    let vars_file = res_dir.join("Vars.inc");
    let mut f2 = File::create(&vars_file).unwrap();
    f2.write_all(b"[Variables]\n@Include=#@#Styles.inc\nPrefix=App\n").unwrap();

    let main_ini = r#"
[Rainmeter]
Update=1000

[Variables]
@Include=#@#Vars.inc
Title=#Prefix# Clock

[MeterTitle]
Meter=String
MeterStyle=StyleBase | StyleAccent
Text=#Title#
"#;

    let config = parse_skin_ini(main_ini, tmp.path()).unwrap();
    assert_eq!(config.variables.get("prefix").unwrap(), "App");
    assert_eq!(config.variables.get("title").unwrap(), "App Clock");

    let meter = config.meters.get("metertitle").unwrap();
    assert_eq!(meter.font_face.as_deref(), Some("Roboto"));
    assert_eq!(meter.font_size, Some(14.0));
    assert_eq!(meter.font_color.as_deref(), Some("255,0,0"));
    assert!(meter.anti_alias);
    assert_eq!(meter.text.as_deref(), Some("App Clock"));
}

#[test]
fn test_circular_include_detection() {
    let tmp = tempdir().unwrap();
    let f1_path = tmp.path().join("a.inc");
    let f2_path = tmp.path().join("b.inc");

    let mut f1 = File::create(&f1_path).unwrap();
    f1.write_all(b"@Include=b.inc\n").unwrap();

    let mut f2 = File::create(&f2_path).unwrap();
    f2.write_all(b"@Include=a.inc\n").unwrap();

    let main_ini = r#"
[Rainmeter]
Update=1000
@Include=a.inc
"#;

    let res = parse_skin_ini(main_ini, tmp.path());
    assert!(matches!(res, Err(pluvia_core::ini::ParseError::CircularInclude(_))));
}

#[test]
fn test_formula_ternary_and_comparisons() {
    let vars = VariableMap::new();
    assert_eq!(eval_formula("(10 > 5) ? 100 : 200", &vars).unwrap(), 100.0);
    assert_eq!(eval_formula("(2 >= 5) ? 100 : 200", &vars).unwrap(), 200.0);
    assert_eq!(eval_formula("(5 == 5) ? 42 : 0", &vars).unwrap(), 42.0);
    assert_eq!(eval_formula("(5 != 5) ? 42 : 0", &vars).unwrap(), 0.0);
}

#[test]
fn test_comments_and_empty_lines() {
    let ini = r#"
; Top level comment
; Another comment line

[Rainmeter]
; Comment inside section
Update=1000

[Variables]
; Comment before var
Font=Arial

[MeterA]
Meter=String
Text=Hello; World
"#;

    let config = parse_skin_ini(ini, Path::new("/dummy")).unwrap();
    assert_eq!(config.update_rate_ms, 1000);
    assert_eq!(config.variables.get("font").unwrap(), "Arial");
    let meter = config.meters.get("metera").unwrap();
    assert_eq!(meter.text.as_deref(), Some("Hello; World"));
}
