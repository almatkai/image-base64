//! Source-compatible syntax/Buffer/header helpers. This is deliberately not a codec.
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};

pub const MAX_INPUT: usize = 1_500_000;
pub const MAX_FILE: usize = 1_000_000;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Image {
    pub uri: String,
    pub label: String,
    pub extension: String,
    pub size: f64,
    pub dimensions: (u32, u32),
    pub bytes: Vec<u8>,
}
pub fn strip_prefix(text: &str) -> &str {
    if let Some(rest) = text.strip_prefix("data:image/") {
        let word = rest
            .bytes()
            .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
            .count();
        if word > 0 && rest[word..].starts_with(";base64,") {
            return &rest[word + 8..];
        }
    }
    text
}
/// Node Buffer's forgiving base64 decoder: low bytes of UTF-16, URL alphabet,
/// ignored non-alphabet, stop at first '=', discard trailing incomplete sextet.
pub fn decode(text: &str) -> Vec<u8> {
    let mut output = Vec::with_capacity(text.len().saturating_mul(3) / 4);
    let (mut acc, mut bits) = (0u32, 0u8);
    for unit in text.encode_utf16() {
        let b = unit as u8;
        let n = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' => break,
            _ => continue,
        };
        acc = (acc << 6) | u32::from(n);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push((acc >> bits) as u8);
        }
    }
    output
}
pub fn valid(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }
    let b64 = if text.starts_with("data:image/") {
        let mut parts = text.split(',');
        parts.next();
        let Some(data) = parts.next() else {
            return false;
        };
        if parts.next().is_some() {
            return false;
        }
        data
    } else {
        text
    };
    if b64.len() % 4 != 0
        || !b64
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"+/=".contains(&b))
    {
        return false;
    }
    b64.find('=')
        .is_none_or(|n| n == b64.len() - 1 || n == b64.len() - 2)
}
pub fn size(text: &str) -> f64 {
    let padding = if text.ends_with("==") {
        2.0
    } else if text.ends_with('=') {
        1.0
    } else {
        0.0
    };
    text.encode_utf16().count() as f64 * 0.75 - padding
}
pub fn format_size(bytes: f64) -> String {
    // toFixed(2) rounds the exact binary f64 to the larger integer at a tie.
    let (divisor, suffix) = if bytes > 1_048_576.0 {
        (1_048_576.0, "MB")
    } else {
        (1024.0, "KB")
    };
    let value = (bytes / divisor).abs();
    let bits = value.to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i32 - 1023 - 52;
    let numerator = u128::from((bits & ((1u64 << 52) - 1)) | (1u64 << 52)) * 100;
    let hundredths = if value == 0.0 || exponent < -127 {
        0
    } else if exponent < 0 {
        let shift = (-exponent) as u32;
        (numerator + (1u128 << (shift - 1))) >> shift
    } else {
        numerator << exponent as u32
    };
    format!(
        "{}{}.{:02} {suffix}",
        if bytes < 0.0 { "-" } else { "" },
        hundredths / 100,
        hundredths % 100
    )
}
fn be16(b: &[u8], n: usize) -> Option<u16> {
    Some(u16::from_be_bytes(b.get(n..n + 2)?.try_into().ok()?))
}
fn be32(b: &[u8], n: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(n..n + 4)?.try_into().ok()?))
}
fn le16(b: &[u8], n: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(n..n + 2)?.try_into().ok()?))
}
fn ascii_eq(b: &[u8], start: usize, s: &[u8]) -> bool {
    b.get(start..start + s.len())
        .is_some_and(|x| x.iter().zip(s).all(|(a, c)| a & 127 == *c))
}
pub fn dimensions(b: &[u8]) -> Option<(u32, u32)> {
    if b.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some((be32(b, 16)?, be32(b, 20)?));
    }
    if b.starts_with(b"\xff\xd8") {
        let mut offset = 2;
        while offset < b.len() {
            if b.get(offset) == Some(&255) && matches!(b.get(offset + 1), Some(0xc0 | 0xc2)) {
                return Some((
                    u32::from(be16(b, offset + 7)?),
                    u32::from(be16(b, offset + 5)?),
                ));
            }
            offset = offset.checked_add(2 + usize::from(be16(b, offset + 2)?))?;
        }
    }
    if ascii_eq(b, 0, b"RIFF") && ascii_eq(b, 8, b"WEBP") && ascii_eq(b, 12, b"VP8") {
        // Original compares THREE bytes; its VP8L branch is unreachable.
        return Some((
            u32::from(le16(b, 26)? & 0x3fff),
            u32::from(le16(b, 28)? & 0x3fff),
        ));
    }
    None
}
pub fn from_base64(original: &str, format: &str) -> Result<Image, String> {
    if original.len() > MAX_INPUT {
        return Err("Base64 input exceeds 1,500,000 UTF-8 bytes".into());
    }
    if original.is_empty() {
        return Err("Please enter a base64 string".into());
    }
    if !["png", "jpeg", "webp"].contains(&format) {
        return Err("Select PNG, JPEG, or WebP".into());
    }
    let clean = original.replace(['\'', '"'], "");
    if !valid(&clean) {
        return Err("Invalid base64 string. Please check your input.".into());
    }
    let uri = if clean.starts_with("data:image/") {
        clean
    } else {
        format!("data:image/{format};base64,{clean}")
    };
    let bytes = decode(strip_prefix(&uri));
    if bytes.len() > MAX_FILE {
        return Err("Decoded data exceeds 1,000,000 bytes".into());
    }
    Ok(Image {
        uri,
        label: String::new(),
        extension: format.into(),
        size: size(original),
        dimensions: dimensions(&decode(strip_prefix(original))).unwrap_or((0, 0)),
        bytes,
    })
}
pub fn from_file(bytes: Vec<u8>, name: String, extension: String) -> Image {
    let uri = format!("data:image/{extension};base64,{}", STANDARD.encode(&bytes));
    let dims = dimensions(&decode(strip_prefix(&uri))).unwrap_or((0, 0));
    Image {
        uri,
        label: name,
        extension,
        size: bytes.len() as f64,
        dimensions: dims,
        bytes,
    }
}
pub fn preview(image: &Image) -> Option<String> {
    let b = &image.bytes;
    let (mime, dims) = if b.starts_with(b"\x89PNG\r\n\x1a\n") {
        ("png", dimensions(b)?)
    } else if b.starts_with(b"\xff\xd8") {
        ("jpeg", dimensions(b)?)
    } else if b.starts_with(b"RIFF") && b.get(8..12) == Some(b"WEBP") {
        let dims = if b.get(12..16) == Some(b"VP8L") {
            let bits = u32::from_le_bytes(b.get(21..25)?.try_into().ok()?);
            ((bits & 0x3fff) + 1, ((bits >> 14) & 0x3fff) + 1)
        } else if b.get(12..16) == Some(b"VP8 ") {
            dimensions(b)?
        } else {
            return None;
        };
        ("webp", dims)
    } else {
        return None;
    };
    if dims.0 == 0
        || dims.1 == 0
        || dims.0 > 8192
        || dims.1 > 8192
        || u64::from(dims.0) * u64::from(dims.1) > 16_000_000
    {
        return None;
    }
    Some(format!(
        "![Converted Image](data:image/{mime};base64,{})",
        STANDARD.encode(b)
    ))
}
