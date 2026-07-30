//! Canonicalizing URLs so the same page saved three ways is one save.

use crate::error::{Error, Result};

/// Query parameters that only track the click, never select content.
const TRACKING_PARAMS: &[&str] = &[
    "fbclid", "gclid", "gbraid", "wbraid", "msclkid", "yclid", "twclid", "igshid", "mc_cid",
    "mc_eid", "si",
];

fn is_tracking_param(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key.starts_with("utm_") || TRACKING_PARAMS.contains(&key.as_str())
}

/// Validate and canonicalize: same article shared via three tracking links
/// must land on one save.
pub(super) fn normalize_url(raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    let mut parsed =
        url::Url::parse(trimmed).map_err(|_| Error::InvalidUrl(trimmed.to_string()))?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err(Error::InvalidUrl(trimmed.to_string()));
    }
    if parsed.query().is_some() {
        let kept: Vec<(String, String)> = parsed
            .query_pairs()
            .filter(|(k, _)| !is_tracking_param(k))
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        if kept.is_empty() {
            parsed.set_query(None);
        } else {
            parsed
                .query_pairs_mut()
                .clear()
                .extend_pairs(kept.iter().map(|(k, v)| (k.as_str(), v.as_str())));
        }
    }
    Ok(parsed.to_string())
}
