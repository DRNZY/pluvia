use pluvia_core::encoding::decode_ini_bytes;

#[test]
fn test_decode_utf8_with_and_without_bom() {
    let plain_utf8 = b"[Rainmeter]\nUpdate=1000\n";
    assert_eq!(decode_ini_bytes(plain_utf8).unwrap(), "[Rainmeter]\nUpdate=1000\n");

    let bom_utf8 = [0xEF, 0xBB, 0xBF, b'[', b'R', b'a', b'i', b'n', b']'];
    assert_eq!(decode_ini_bytes(&bom_utf8).unwrap(), "[Rain]");
}

#[test]
fn test_decode_utf16_le_with_bom() {
    // UTF-16 LE BOM [0xFF, 0xFE] + "[Rainmeter]"
    let mut utf16 = vec![0xFF, 0xFE];
    for ch in "[Rainmeter]".encode_utf16() {
        utf16.extend_from_slice(&ch.to_le_bytes());
    }
    assert_eq!(decode_ini_bytes(&utf16).unwrap(), "[Rainmeter]");
}

#[test]
fn test_decode_utf16_be_with_bom() {
    // UTF-16 BE BOM [0xFE, 0xFF] + "[Rainmeter]"
    let mut utf16 = vec![0xFE, 0xFF];
    for ch in "[Rainmeter]".encode_utf16() {
        utf16.extend_from_slice(&ch.to_be_bytes());
    }
    assert_eq!(decode_ini_bytes(&utf16).unwrap(), "[Rainmeter]");
}

#[test]
fn test_decode_windows_1252_ansi() {
    // 0x93 and 0x94 are smart quotes in CP1252, invalid in UTF-8
    let cp1252_bytes = vec![b'T', b'e', b'x', b't', b'=', 0x93, b'H', b'i', 0x94];
    let decoded = decode_ini_bytes(&cp1252_bytes).unwrap();
    assert!(decoded.starts_with("Text="));
    assert!(decoded.contains('“') || decoded.contains('"'));
}

#[test]
fn test_decode_empty_bytes() {
    assert_eq!(decode_ini_bytes(b"").unwrap(), "");
}
