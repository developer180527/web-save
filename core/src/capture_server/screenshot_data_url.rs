//! Decoding the viewport screenshot the extension sends as a data URL.
//!
//! Only a fallback: the extension attaches one when the page declares no
//! cover image of its own, and it is never allowed to replace a thumbnail the
//! vault already has.

/// Ceiling on a decoded screenshot. The request body cap is larger because
/// base64 inflates by a third and the JSON carries metadata too.
pub(super) const MAX_SCREENSHOT_BYTES: usize = 6 * 1024 * 1024;

/// `data:image/jpeg;base64,…` → (bytes, extension).
///
/// Returns `None` for anything that isn't a supported image type, doesn't
/// decode, or is empty or oversized — a bad screenshot must never fail the
/// save it came with.
pub(super) fn decode(data_url: &str) -> Option<(Vec<u8>, &'static str)> {
    use base64::Engine;
    let rest = data_url.strip_prefix("data:")?;
    let (mime, b64) = rest.split_once(";base64,")?;
    let ext = match mime.to_ascii_lowercase().as_str() {
        "image/jpeg" | "image/jpg" => "jpg",
        "image/png" => "png",
        "image/webp" => "webp",
        _ => return None,
    };
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .ok()?;
    (!bytes.is_empty() && bytes.len() <= MAX_SCREENSHOT_BYTES).then_some((bytes, ext))
}

#[cfg(test)]
mod tests {
    use super::decode;

    #[test]
    fn decodes_supported_image_types() {
        // "hi" in base64.
        let (bytes, ext) = decode("data:image/png;base64,aGk=").unwrap();
        assert_eq!(bytes, b"hi");
        assert_eq!(ext, "png");
        assert_eq!(decode("data:image/jpeg;base64,aGk=").unwrap().1, "jpg");
    }

    #[test]
    fn rejects_anything_that_is_not_a_plain_image() {
        // An SVG would be script-capable; a data URL is not a bare payload.
        assert!(decode("data:image/svg+xml;base64,aGk=").is_none());
        assert!(decode("data:text/html;base64,aGk=").is_none());
        assert!(decode("https://example.com/x.png").is_none());
        assert!(decode("data:image/png;base64,!!!not base64!!!").is_none());
        assert!(decode("data:image/png;base64,").is_none(), "empty");
    }
}
