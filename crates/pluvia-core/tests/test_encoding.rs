use pluvia_core::encoding::{decode_ini_bytes, EncodingError};

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
    // 0x93 and 0x94 are smart quotes in CP1252 (“ and ”), invalid in UTF-8
    let cp1252_bytes = vec![b'T', b'e', b'x', b't', b'=', 0x93, b'H', b'i', 0x94];
    let decoded = decode_ini_bytes(&cp1252_bytes).unwrap();
    assert!(decoded.starts_with("Text="));
    assert!(decoded.contains('“'));
    assert!(decoded.contains('”'));
    assert_eq!(decoded, "Text=“Hi”");
}

#[test]
fn test_decode_empty_bytes() {
    assert_eq!(decode_ini_bytes(b"").unwrap(), "");
}

#[test]
fn test_decode_malformed_utf16_le() {
    // UTF-16 LE BOM followed by an odd number of bytes (truncated code unit)
    let malformed_le = vec![0xFF, 0xFE, 0x41];
    assert_eq!(decode_ini_bytes(&malformed_le), Err(EncodingError::DecodingFailed));

    // UTF-16 LE BOM followed by unpaired surrogate
    let unpaired_surrogate_le = vec![0xFF, 0xFE, 0x00, 0xD8];
    assert_eq!(decode_ini_bytes(&unpaired_surrogate_le), Err(EncodingError::DecodingFailed));
}

#[test]
fn test_decode_malformed_utf16_be() {
    // UTF-16 BE BOM followed by an odd number of bytes (truncated code unit)
    let malformed_be = vec![0xFE, 0xFF, 0x41];
    assert_eq!(decode_ini_bytes(&malformed_be), Err(EncodingError::DecodingFailed));

    // UTF-16 BE BOM followed by unpaired surrogate
    let unpaired_surrogate_be = vec![0xFE, 0xFF, 0xD8, 0x00];
    assert_eq!(decode_ini_bytes(&unpaired_surrogate_be), Err(EncodingError::DecodingFailed));
}
