//! Standardised filename generation for nuclei POC templates.
//!
//! Naming priority:
//! 1. `id` field → sanitised (non-alnum → dash, dedupe dashes)
//! 2. `name` field → lowercased + sanitised
//! 3. Fallback → `poc-{first 12 chars of sha256}.yaml`

use regex::Regex;
use std::sync::LazyLock;

static DASH_COLLAPSE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"-+").unwrap());

/// Generate a standard `.yaml` filename from POC metadata.
///
/// `id` and `name` are optional; `content_hash` is the SHA-256 hex string used
/// as a fallback when both are missing.
pub fn standard_filename(id: Option<&str>, name: Option<&str>, content_hash: &str) -> String {
    // 1. id field
    if let Some(raw_id) = id {
        let s: String = raw_id
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
            .collect();
        let cleaned = DASH_COLLAPSE_RE.replace_all(&s, "-");
        let trimmed = cleaned.trim_matches('-');
        if !trimmed.is_empty() {
            return format!("{}.yaml", trimmed);
        }
    }

    // 2. name field
    if let Some(raw_name) = name {
        let s: String = raw_name
            .to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '-' })
            .collect();
        let cleaned = DASH_COLLAPSE_RE.replace_all(&s, "-");
        let trimmed = cleaned.trim_matches('-');
        if !trimmed.is_empty() {
            return format!("{}.yaml", trimmed);
        }
    }

    // 3. fallback — hash prefix
    format!("poc-{}.yaml", &content_hash[..12.min(content_hash.len())])
}
