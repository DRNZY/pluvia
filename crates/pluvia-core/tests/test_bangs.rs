use pluvia_core::bangs::{parse_bangs, tokenize_args, Bang};

#[test]
fn test_tokenize_args_with_quotes() {
    let args = tokenize_args(r#"!SetVariable MyVar "Hello World" ConfigName"#);
    assert_eq!(args, vec!["!SetVariable", "MyVar", "Hello World", "ConfigName"]);
}

#[test]
fn test_parse_single_unbracketed_bang() {
    let bangs = parse_bangs("!SetVariable Scale 1.5");
    assert_eq!(
        bangs,
        vec![Bang::SetVariable {
            name: "Scale".to_string(),
            value: "1.5".to_string(),
            config: None,
        }]
    );
}

#[test]
fn test_parse_multiple_bracketed_bangs() {
    let bangs = parse_bangs(r#"[!SetVariable Scale 1.2][!UpdateMeter *][!Redraw]"#);
    assert_eq!(
        bangs,
        vec![
            Bang::SetVariable {
                name: "Scale".to_string(),
                value: "1.2".to_string(),
                config: None,
            },
            Bang::UpdateMeter {
                name: "*".to_string(),
                config: None,
            },
            Bang::Redraw { config: None }
        ]
    );
}

#[test]
fn test_parse_write_key_value() {
    let bangs = parse_bangs(r##"[!WriteKeyValue Variables Scale "2.0" "#@#Settings.inc"]"##);
    assert_eq!(bangs.len(), 1);
    match &bangs[0] {
        Bang::WriteKeyValue { section, key, value, file } => {
            assert_eq!(section, "Variables");
            assert_eq!(key, "Scale");
            assert_eq!(value, "2.0");
            assert_eq!(file.as_ref().unwrap().to_str().unwrap(), "#@#Settings.inc");
        }
        _ => panic!("Expected WriteKeyValue"),
    }
}

#[test]
fn test_parse_external_execute() {
    let bangs = parse_bangs(r#"["https://google.com"]"#);
    assert_eq!(bangs, vec![Bang::Execute("https://google.com".to_string())]);
}

#[test]
fn test_write_key_value_to_file_persistence() {
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("Settings.inc");

    std::fs::write(
        &file_path,
        "[Variables]\nScale=1.0\nColor=255,255,255\n",
    ).unwrap();

    pluvia_core::bangs::write_key_value_to_file(&file_path, "Variables", "Scale", "1.5").unwrap();

    let updated = std::fs::read_to_string(&file_path).unwrap();
    assert!(updated.contains("Scale=1.5"));
    assert!(updated.contains("Color=255,255,255"));
}
