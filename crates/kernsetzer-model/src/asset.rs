//! Images that belong to a book, referenced by `Block::Image::asset`.
//!
//! An asset is its bytes plus what the bytes say about themselves (format,
//! pixel size, stored DPI). The metadata is always derived from the bytes,
//! never stored next to them, so it cannot go stale: a `.kernsetzer` file holds
//! only `assets/<id>`.

/// An image file, e.g. `assets/image1.png` in a project.
#[derive(Debug, Clone, PartialEq)]
pub struct Asset {
    /// File name under `assets/` in the project, e.g. `image1.png`.
    pub id: String,
    pub media_type: String,
    pub bytes: Vec<u8>,
    /// Pixel size from the file header, if it could be read.
    pub width_px: Option<u32>,
    pub height_px: Option<u32>,
    /// Resolution stored in the file (PNG `pHYs`, JPEG JFIF), if any.
    pub dpi: Option<f32>,
}

impl Asset {
    /// An asset with metadata read from `bytes`. Content that is not a
    /// supported image gets `application/octet-stream` and no size.
    pub fn new(id: impl Into<String>, bytes: Vec<u8>) -> Self {
        let media_type = match ImageFormat::sniff(&bytes) {
            ImageFormat::Supported { media_type, .. } => media_type,
            ImageFormat::Unsupported(_) => "application/octet-stream",
        };
        let info = header_info(&bytes);
        Self {
            id: id.into(),
            media_type: media_type.into(),
            width_px: info.map(|i| i.0),
            height_px: info.map(|i| i.1),
            dpi: info.and_then(|i| i.2),
            bytes,
        }
    }

    /// Whether `id` can be used as a file name under `assets/`: a plain
    /// name (no directories, no `..`, no leading dot) of portable
    /// characters.
    pub fn is_valid_id(id: &str) -> bool {
        !id.is_empty()
            && id.len() <= 255
            && !id.starts_with('.')
            && id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    }
}

/// What kind of image a file is, judged by content, not extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    /// A format books can use: PNG, JPEG, GIF, WebP, SVG.
    Supported {
        media_type: &'static str,
        /// File extension to use for the asset id.
        ext: &'static str,
    },
    /// Anything else, with a name to show the author (`"TIFF"`).
    Unsupported(&'static str),
}

impl ImageFormat {
    /// Identify an image by its magic bytes.
    pub fn sniff(b: &[u8]) -> Self {
        let ok = |media_type, ext| ImageFormat::Supported { media_type, ext };
        match b {
            [0x89, b'P', b'N', b'G', ..] => ok("image/png", "png"),
            [0xFF, 0xD8, 0xFF, ..] => ok("image/jpeg", "jpg"),
            [b'G', b'I', b'F', b'8', ..] => ok("image/gif", "gif"),
            [
                b'R',
                b'I',
                b'F',
                b'F',
                _,
                _,
                _,
                _,
                b'W',
                b'E',
                b'B',
                b'P',
                ..,
            ] => ok("image/webp", "webp"),
            [0xD7, 0xCD, 0xC6, 0x9A, ..] | [0x01 | 0x02, 0x00, 0x09, 0x00, ..] => {
                Self::Unsupported("WMF")
            }
            [0x01, 0x00, 0x00, 0x00, ..] if b.get(40..44) == Some(b" EMF") => {
                Self::Unsupported("EMF")
            }
            [b'I', b'I', 0x2A, 0x00, ..] | [b'M', b'M', 0x00, 0x2A, ..] => {
                Self::Unsupported("TIFF")
            }
            [b'B', b'M', ..] => Self::Unsupported("BMP"),
            _ => {
                let head = String::from_utf8_lossy(&b[..b.len().min(1024)]);
                if head.contains("<svg") {
                    ok("image/svg+xml", "svg")
                } else {
                    Self::Unsupported("an unknown")
                }
            }
        }
    }
}

/// (width, height, dpi) from the header of a PNG, JPEG, GIF or WebP.
fn header_info(b: &[u8]) -> Option<(u32, u32, Option<f32>)> {
    let be16 = |i: usize| {
        b.get(i..i + 2)
            .map(|s| u32::from(u16::from_be_bytes([s[0], s[1]])))
    };
    let be32 = |i: usize| {
        b.get(i..i + 4)
            .map(|s| u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
    };
    let le16 = |i: usize| {
        b.get(i..i + 2)
            .map(|s| u32::from(u16::from_le_bytes([s[0], s[1]])))
    };
    let le24 = |i: usize| {
        b.get(i..i + 3)
            .map(|s| u32::from(s[0]) | u32::from(s[1]) << 8 | u32::from(s[2]) << 16)
    };
    let ImageFormat::Supported { media_type, .. } = ImageFormat::sniff(b) else {
        return None;
    };
    match media_type {
        "image/png" => {
            let (w, h) = (be32(16)?, be32(20)?);
            // Walk chunks for pHYs (pixels per metre when unit == 1).
            let mut i = 8;
            let mut dpi = None;
            while let (Some(len), Some(kind)) = (be32(i), b.get(i + 4..i + 8)) {
                match kind {
                    b"pHYs" if b.get(i + 16) == Some(&1) => {
                        dpi = be32(i + 8).map(|ppm| ppm as f32 * 0.0254);
                        break;
                    }
                    b"IDAT" | b"IEND" => break,
                    _ => i = i.checked_add(12 + len as usize)?,
                }
            }
            Some((w, h, dpi))
        }
        "image/jpeg" => {
            let mut i = 2;
            let mut dpi = None;
            loop {
                if b.get(i) != Some(&0xFF) {
                    return None;
                }
                let marker = *b.get(i + 1)?;
                if marker == 0xFF {
                    i += 1; // fill byte
                    continue;
                }
                let len = be16(i + 2)? as usize;
                let data = i + 4;
                match marker {
                    // APP0 JFIF: units (1 = dpi, 2 = dots per cm), X density.
                    0xE0 if b.get(data..data + 5) == Some(b"JFIF\0") => {
                        let x = be16(data + 8)? as f32;
                        dpi = match b.get(data + 7) {
                            Some(1) => Some(x),
                            Some(2) => Some(x * 2.54),
                            _ => None,
                        };
                    }
                    // Start of frame (not DHT, JPG, DAC).
                    0xC0..=0xCF if !matches!(marker, 0xC4 | 0xC8 | 0xCC) => {
                        let h = be16(data + 1)?;
                        let w = be16(data + 3)?;
                        return Some((w, h, dpi.filter(|d| *d > 1.0)));
                    }
                    _ => {}
                }
                i = data.checked_add(len.checked_sub(2)?)?;
            }
        }
        "image/gif" => Some((le16(6)?, le16(8)?, None)),
        "image/webp" => match b.get(12..16)? {
            b"VP8X" => Some((le24(24)? + 1, le24(27)? + 1, None)),
            b"VP8L" => {
                let bits = b.get(21..25)?;
                let v = u32::from_le_bytes([bits[0], bits[1], bits[2], bits[3]]);
                Some(((v & 0x3FFF) + 1, ((v >> 14) & 0x3FFF) + 1, None))
            }
            b"VP8 " => Some((le16(26)? & 0x3FFF, le16(28)? & 0x3FFF, None)),
            _ => None,
        },
        _ => None,
    }
}

/// Test images for this crate and its dependents' tests.
#[doc(hidden)]
pub mod testdata {
    /// A valid 3×2 PNG with a 300 dpi pHYs chunk (CRCs are not checked).
    pub fn png_3x2_300dpi() -> Vec<u8> {
        let mut b = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        b.extend_from_slice(&13u32.to_be_bytes());
        b.extend_from_slice(b"IHDR");
        b.extend_from_slice(&3u32.to_be_bytes());
        b.extend_from_slice(&2u32.to_be_bytes());
        b.extend_from_slice(&[8, 2, 0, 0, 0]);
        b.extend_from_slice(&[0; 4]);
        b.extend_from_slice(&9u32.to_be_bytes());
        b.extend_from_slice(b"pHYs");
        b.extend_from_slice(&11811u32.to_be_bytes());
        b.extend_from_slice(&11811u32.to_be_bytes());
        b.push(1);
        b.extend_from_slice(&[0; 4]);
        b.extend_from_slice(&0u32.to_be_bytes());
        b.extend_from_slice(b"IEND");
        b.extend_from_slice(&[0; 4]);
        b
    }
}

#[cfg(test)]
mod tests {
    use super::testdata::png_3x2_300dpi;
    use super::*;

    #[test]
    fn png_header() {
        let (w, h, dpi) = header_info(&png_3x2_300dpi()).unwrap();
        assert_eq!((w, h), (3, 2));
        assert!((dpi.unwrap() - 300.0).abs() < 0.1);
    }

    #[test]
    fn jpeg_header() {
        let mut b = vec![0xFF, 0xD8];
        // APP0 JFIF, 72 dpi
        b.extend_from_slice(&[0xFF, 0xE0, 0x00, 0x10]);
        b.extend_from_slice(b"JFIF\0");
        b.extend_from_slice(&[1, 1, 1, 0, 72, 0, 72, 0, 0]);
        // SOF0: precision, height 480, width 640
        b.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x11, 8, 0x01, 0xE0, 0x02, 0x80]);
        b.extend_from_slice(&[0; 12]);
        assert_eq!(header_info(&b), Some((640, 480, Some(72.0))));
    }

    #[test]
    fn truncated_headers_do_not_panic() {
        let png = png_3x2_300dpi();
        for n in 0..png.len() {
            let _ = header_info(&png[..n]);
        }
        for b in [
            &[0xFF, 0xD8, 0xFF][..],
            &[0xFF, 0xD8, 0xFF, 0xC0, 0, 1][..],
            b"GIF8",
            b"RIFF0000WEBPVP8X",
        ] {
            let _ = header_info(b);
        }
    }

    #[test]
    fn formats_by_content() {
        assert_eq!(
            ImageFormat::sniff(b"GIF89a.."),
            ImageFormat::Supported {
                media_type: "image/gif",
                ext: "gif"
            }
        );
        assert_eq!(
            ImageFormat::sniff(&[0xD7, 0xCD, 0xC6, 0x9A, 0]),
            ImageFormat::Unsupported("WMF")
        );
        assert_eq!(
            ImageFormat::sniff(
                br#"<?xml version="1.0"?><svg xmlns="http://www.w3.org/2000/svg"/>"#
            ),
            ImageFormat::Supported {
                media_type: "image/svg+xml",
                ext: "svg"
            }
        );
    }

    #[test]
    fn new_reads_metadata() {
        let a = Asset::new("p.png", png_3x2_300dpi());
        assert_eq!(a.media_type, "image/png");
        assert_eq!((a.width_px, a.height_px), (Some(3), Some(2)));
        let junk = Asset::new("x.bin", b"hello".to_vec());
        assert_eq!(junk.media_type, "application/octet-stream");
        assert_eq!(junk.width_px, None);
    }

    #[test]
    fn asset_ids() {
        for ok in ["image1.png", "Cover_2-final.jpg", "a"] {
            assert!(Asset::is_valid_id(ok), "{ok}");
        }
        for bad in [
            "", ".hidden", "../x.png", "a/b.png", "a\\b.png", "ü.png", "a b.png",
        ] {
            assert!(!Asset::is_valid_id(bad), "{bad}");
        }
    }
}
