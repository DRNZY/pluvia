use pluvia_core::ini::parse_skin_file;
use std::path::PathBuf;

#[test]
fn test_parse_cleartext_pure() {
    let base_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();

    let skin_path = base_dir.join("tests/fixtures/real_world_skins/cleartext/Cleartext Pure.ini");
    println!("Testing parsing: {}", skin_path.display());

    let skin_config = parse_skin_file(&skin_path).expect("Failed to parse Cleartext Pure.ini");
    println!("Successfully parsed skin. Update: {}", skin_config.update_rate_ms);
    println!("Meters: {}", skin_config.meters.len());
    println!("Measures: {}", skin_config.measures.len());
}
