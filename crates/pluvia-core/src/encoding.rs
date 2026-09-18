use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum EncodingError {
    #[error("Failed to decode raw bytes with detected charset")]
    DecodingFailed,
}

pub fn decode_ini_bytes(raw: &[u8]) -> Result<String, EncodingError> {
    if raw.is_empty() {
        return Ok(String::new());
    }

    // 1. Check for UTF-16 LE BOM: 0xFF, 0xFE
    if raw.len() >= 2 && raw[0] == 0xFF && raw[1] == 0xFE {
        let (cow, _, malformed) = encoding_rs::UTF_16LE.decode(&raw[2..]);
        if !malformed {
            return Ok(cow.into_owned());
        }
    }

    // 2. Check for UTF-16 BE BOM: 0xFE, 0xFF
    if raw.len() >= 2 && raw[0] == 0xFE && raw[1] == 0xFF {
        let (cow, _, malformed) = encoding_rs::UTF_16BE.decode(&raw[2..]);
        if !malformed {
            return Ok(cow.into_owned());
        }
    }

    // 3. Check for UTF-8 BOM: 0xEF, 0xBB, 0xBF
    let slice = if raw.len() >= 3 && raw[0] == 0xEF && raw[1] == 0xBB && raw[2] == 0xBF {
        &raw[3..]
    } else {
        raw
    };

    // 4. Try UTF-8 directly
    if let Ok(valid_str) = std::str::from_utf8(slice) {
        return Ok(valid_str.to_string());
    }

    // 5. Fallback to Windows-1252 (CP1252)
    let (cow, _, _) = encoding_rs::WINDOWS_1252.decode(slice);
    Ok(cow.into_owned())
}
