use pluvia_core::render::pango_text::FontRegistry;
use std::path::Path;

#[test]
fn test_skin_font_discovery_and_registration() {
    let cleartext_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("tests/fixtures/real_world_skins/cleartext");

    if cleartext_dir.exists() {
        let count = FontRegistry::register_skin_fonts(&cleartext_dir);
        assert!(count >= 5, "Expected at least 5 fonts to be registered, found {}", count);
    }
}
