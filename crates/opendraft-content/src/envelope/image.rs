//! The header's thumbnail (specification 5.1, 5.4): base64 PNG, capped, and
//! never trusted. This checks the bytes' shape without decoding any pixels;
//! the app decodes with its own size limits and re-renders its own thumbnail.

use std::fmt;

/// The decoded PNG may be at most this many bytes.
pub const MAX_THUMBNAIL_BYTES: usize = 64 * 1024;
/// And at most this many pixels on a side.
pub const MAX_THUMBNAIL_SIDE: u32 = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbnailError {
    NotBase64,
    TooLarge,
    NotPng,
    TooManyPixels,
}

impl fmt::Display for ThumbnailError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ThumbnailError::NotBase64 => write!(f, "is not base64"),
            ThumbnailError::TooLarge => {
                write!(f, "is larger than {} KB", MAX_THUMBNAIL_BYTES / 1024)
            }
            ThumbnailError::NotPng => write!(f, "is not a PNG image"),
            ThumbnailError::TooManyPixels => {
                write!(f, "is more than {MAX_THUMBNAIL_SIDE} pixels on a side")
            }
        }
    }
}

const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// Decodes and checks a thumbnail, returning the PNG bytes.
pub fn decode_thumbnail(text: &str) -> Result<Vec<u8>, ThumbnailError> {
    // Cheap bound before allocating: base64 is 4 characters for 3 bytes.
    if text.len() > MAX_THUMBNAIL_BYTES / 3 * 4 + 4 {
        return Err(ThumbnailError::TooLarge);
    }
    let png = decode_base64(text).ok_or(ThumbnailError::NotBase64)?;
    if png.len() > MAX_THUMBNAIL_BYTES {
        return Err(ThumbnailError::TooLarge);
    }
    let header_ok = png.len() >= 24 && png[..8] == PNG_SIGNATURE && &png[12..16] == b"IHDR";
    if !header_ok {
        return Err(ThumbnailError::NotPng);
    }
    let side = |at: usize| u32::from_be_bytes([png[at], png[at + 1], png[at + 2], png[at + 3]]);
    let (width, height) = (side(16), side(20));
    if width == 0 || height == 0 {
        return Err(ThumbnailError::NotPng);
    }
    if width > MAX_THUMBNAIL_SIDE || height > MAX_THUMBNAIL_SIDE {
        return Err(ThumbnailError::TooManyPixels);
    }
    Ok(png)
}

/// Standard alphabet, `=` padding required, no whitespace.
pub fn decode_base64(text: &str) -> Option<Vec<u8>> {
    let bytes = text.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    let chunks = bytes.len() / 4;
    for (i, chunk) in bytes.as_chunks::<4>().0.iter().enumerate() {
        let last = i + 1 == chunks;
        let padding = chunk.iter().rev().take_while(|&&b| b == b'=').count();
        if padding > 2 || (padding > 0 && !last) {
            return None;
        }
        let mut n = 0u32;
        for &b in &chunk[..4 - padding] {
            n = (n << 6) | sextet(b)?;
        }
        n <<= 6 * padding as u32;
        let triple = n.to_be_bytes();
        out.extend_from_slice(&triple[1..4 - padding]);
        // The unused low bits of the last character must be zero, so one
        // value has one spelling.
        if padding > 0 && n & ((1 << (8 * padding)) - 1) != 0 {
            return None;
        }
    }
    Some(out)
}

fn sextet(b: u8) -> Option<u32> {
    match b {
        b'A'..=b'Z' => Some(u32::from(b - b'A')),
        b'a'..=b'z' => Some(u32::from(b - b'a') + 26),
        b'0'..=b'9' => Some(u32::from(b - b'0') + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

/// The canonical spelling `decode_base64` accepts.
pub fn encode_base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let mut n = 0u32;
        for i in 0..3 {
            n = (n << 8) | u32::from(*chunk.get(i).unwrap_or(&0));
        }
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// A PNG header with this size and nothing after it: enough for the checks.
#[cfg(test)]
pub(crate) fn fake_png(width: u32, height: u32) -> Vec<u8> {
    let mut png = PNG_SIGNATURE.to_vec();
    png.extend_from_slice(&13u32.to_be_bytes());
    png.extend_from_slice(b"IHDR");
    png.extend_from_slice(&width.to_be_bytes());
    png.extend_from_slice(&height.to_be_bytes());
    png.extend_from_slice(&[8, 6, 0, 0, 0]);
    png
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trips_every_length() {
        for len in 0..40usize {
            let data: Vec<u8> = (0..len).map(|i| (i * 37 + 11) as u8).collect();
            assert_eq!(decode_base64(&encode_base64(&data)), Some(data), "{len}");
        }
    }

    #[test]
    fn base64_refuses_what_is_not_canonical() {
        for bad in [
            "Zg=", "Zg", "Z===", "Zm9v\n", "Zm 9", "Zg==Zg==", "Zh==", "Zm9=", "Zm-v",
        ] {
            assert_eq!(decode_base64(bad), None, "{bad:?}");
        }
        assert_eq!(decode_base64("Zg=="), Some(b"f".to_vec()));
    }

    #[test]
    fn a_small_png_is_accepted() {
        let png = fake_png(256, 128);
        assert_eq!(decode_thumbnail(&encode_base64(&png)), Ok(png));
    }

    #[test]
    fn size_and_shape_are_checked() {
        assert_eq!(
            decode_thumbnail(&encode_base64(&fake_png(257, 10))),
            Err(ThumbnailError::TooManyPixels)
        );
        assert_eq!(
            decode_thumbnail(&encode_base64(&fake_png(0, 10))),
            Err(ThumbnailError::NotPng)
        );
        assert_eq!(
            decode_thumbnail(&encode_base64(b"GIF89a not a png at all......")),
            Err(ThumbnailError::NotPng)
        );
        assert_eq!(decode_thumbnail("!!!!"), Err(ThumbnailError::NotBase64));
        let mut big = fake_png(10, 10);
        big.resize(MAX_THUMBNAIL_BYTES + 1, 0);
        assert_eq!(
            decode_thumbnail(&encode_base64(&big)),
            Err(ThumbnailError::TooLarge)
        );
        // A huge string is refused before it is decoded.
        assert_eq!(
            decode_thumbnail(&"A".repeat(10_000_000)),
            Err(ThumbnailError::TooLarge)
        );
    }
}
