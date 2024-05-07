//! YAML / Nuclei-template helpers.
//!
//! Centralises extraction of CVE/CNVD identifiers, severity normalisation,
//! and format validation so every stage uses identical rules.

use regex::Regex;
use serde_yaml::Value;
use std::sync::LazyLock;

// ---------------------------------------------------------------------------
// regex helpers (compiled once)
// ---------------------------------------------------------------------------

static CVE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)CVE-\d{4}-\d+").unwrap());

static CNVD_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)CNVD-\d{4}-\d+").unwrap());

static CNNVD_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)CNNVD-\d{4}-\d+").unwrap());

// ---------------------------------------------------------------------------
// ID extraction
// ---------------------------------------------------------------------------

/// Extract the first CVE ID found in `text`, uppercased.
pub fn extract_cve(text: &str) -> Option<String> {
    CVE_RE.find(text).map(|m| m.as_str().to_uppercase())
}

/// Extract the first CNVD ID found in `text`, uppercased.
pub fn extract_cnvd(text: &str) -> Option<String> {
    CNVD_RE.find(text).map(|m| m.as_str().to_uppercase())
}

/// Extract the first CNNVD ID found in `text`, uppercased.
pub fn extract_cnnvd(text: &str) -> Option<String> {
    CNNVD_RE.find(text).map(|m| m.as_str().to_uppercase())
}

// ---------------------------------------------------------------------------
// Severity
// ---------------------------------------------------------------------------

/// Official valid nuclei severities (lowercase).
pub const VALID_SEVERITIES: &[&str] = &["info", "low", "medium", "high", "critical", "unknown"];

/// Normalize a severity string to lowercase. Returns `None` for empty input.
pub fn normalize_severity(raw: &str) -> Option<String> {
    let s = raw.trim();
    if s.is_empty() { None } else { Some(s.to_lowercase()) }
}

/// Check whether the raw severity string _as-read-from-YAML_ had casing issues.
/// Returns `true` when the value needs auto-fix (e.g. "Critical", "HIGH").
pub fn has_severity_casing_issue(raw: &str) -> bool {
    let lower = raw.to_lowercase();
    VALID_SEVERITIES.contains(&lower.as_str()) && raw != lower
}

// ---------------------------------------------------------------------------
// CVE/CNVD from YAML info.classification subtree
// ---------------------------------------------------------------------------

/// Extract CVE from `info.classification.cve-id` (string or sequence).
pub fn extract_cve_from_info(info: &Value) -> Option<String> {
    let cls = info.get("classification")?;
    let cve_val = cls.get("cve-id")?;
    match cve_val {
        Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() { None } else { Some(trimmed.to_uppercase()) }
        }
        Value::Sequence(seq) => seq.iter()
            .filter_map(|v| v.as_str())
            .find(|s| !s.is_empty())
            .map(|s| s.trim().to_uppercase()),
        _ => None,
    }
}

/// Extract CNVD from `info.classification.cnvd-id` (string or sequence).
pub fn extract_cnvd_from_info(info: &Value) -> Option<String> {
    let cls = info.get("classification")?;
    let cnvd_val = cls.get("cnvd-id")?;
    match cnvd_val {
        Value::String(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() { None } else { Some(trimmed.to_uppercase()) }
        }
        Value::Sequence(seq) => seq.iter()
            .filter_map(|v| v.as_str())
            .find(|s| !s.is_empty())
            .map(|s| s.trim().to_uppercase()),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Quick structural check: is this a plausible nuclei template?
// ---------------------------------------------------------------------------

/// Fast pre-filter to identify non-nuclei YAML files (docker-compose.yml,
/// Ansible playbooks, Kubernetes manifests, etc.) that ended up in the
/// collection only because they happen to have a `.yaml` extension.
///
/// Returns `(is_template, reason_if_not)`.
/// This is intentionally lenient — it only rejects files that clearly
/// cannot be nuclei templates.  Doubtful cases pass through so later
/// stages (auto-fix + nuclei binary validation) can make the final call.
pub fn is_nuclei_template(content: &str) -> (bool, String) {
    // 1. Empty / whitespace-only
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return (false, "empty file".into());
    }

    // 2. Valid YAML?
    let yaml: Value = match serde_yaml::from_str(content) {
        Ok(v) => v,
        Err(e) => return (false, format!("invalid YAML: {}", e)),
    };

    // 3. Must have a top-level "id" key — the single most universal
    //    nuclei template marker
    if yaml.get("id").is_none() {
        return (false, "missing top-level 'id' field".into());
    }

    // 4. Must have at least one recognised protocol / execution block.
    //    We intentionally include ALL protocol keys nuclei understands
    //    (including deprecated `http:` and `network:`) to avoid
    //    false-negatives on older templates.
    const PROTOCOL_KEYS: &[&str] = &[
        "requests", "http", "tcp", "dns", "file", "headless",
        "network", "websocket", "ssl", "javascript", "code", "workflows",
    ];
    let has_protocol = PROTOCOL_KEYS.iter().any(|k| yaml.get(*k).is_some());
    if !has_protocol {
        return (false, "missing protocol field (requests/http/tcp/…)".into());
    }

    (true, String::new())
}

// ---------------------------------------------------------------------------
// Nuclei format validation
// ---------------------------------------------------------------------------

/// Result of validating a single nuclei YAML template.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

/// Validate the top-level structure of a nuclei template.
///
/// `has_requests` / `has_http` / `has_matchers` / `request_count` should come
/// from the already-parsed feature struct (or be computed separately).
pub fn validate_nuclei_format(
    yaml: &Value,
    has_http: bool,
    has_requests: bool,
    has_matchers: bool,
    request_count: usize,
) -> ValidationResult {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    // --- required top-level fields ---
    if yaml.get("id").is_none() {
        errors.push("missing required field: id".into());
    }
    if yaml.get("info").is_none() {
        errors.push("missing required field: info".into());
    }

    // --- must have at least one protocol field ---
    let has_protocol = yaml.get("requests").is_some()
        || yaml.get("http").is_some()
        || yaml.get("tcp").is_some()
        || yaml.get("dns").is_some()
        || yaml.get("file").is_some()
        || yaml.get("headless").is_some()
        || yaml.get("network").is_some()
        || yaml.get("websocket").is_some()
        || yaml.get("ssl").is_some()
        || yaml.get("javascript").is_some()
        || yaml.get("code").is_some()
        || yaml.get("workflows").is_some();

    if !has_protocol {
        errors.push("missing protocol field (requests/http/tcp/etc.)".into());
    }

    // --- info block ---
    if let Some(info) = yaml.get("info") {
        if info.get("name").is_none() {
            warnings.push("info.name is missing".into());
        }
        if info.get("severity").is_none() {
            warnings.push("info.severity is missing".into());
        } else if let Some(sev) = info.get("severity").and_then(|v| v.as_str()) {
            let lower = sev.to_lowercase();
            if !VALID_SEVERITIES.contains(&lower.as_str()) {
                warnings.push(format!("invalid severity: {}", sev));
            }
        }
        if info.get("author").is_none() {
            warnings.push("info.author is missing".into());
        }
    }

    // --- deprecated `http:` vs `requests:` ---
    if has_http && !has_requests {
        warnings.push("uses deprecated 'http:' field, prefer 'requests:'".into());
    }

    // --- matchers ---
    if !has_matchers && request_count > 0 {
        warnings.push("no matchers found in requests".into());
    }

    ValidationResult {
        is_valid: errors.is_empty(),
        errors,
        warnings,
    }
}

// ---------------------------------------------------------------------------
// Auto-fix common format issues
// ---------------------------------------------------------------------------

/// Statistics for auto-fix operations.
#[derive(Debug, Default, Clone)]
pub struct FixStats {
    pub severity_casing: usize,
    pub severity_empty: usize,
    pub id_spaces: usize,
    pub total_fixed: usize,
}

/// Auto-fix common format issues in nuclei YAML content.
///
/// Fixes:
/// 1. Severity casing (Critical → critical, HIGH → high)
/// 2. Empty severity → inferred from context or default "info"
/// 3. CVE/CNVD/CNNVD ID spaces (CVE 2020-6171 → CVE-2020-6171)
///
/// Returns (fixed_content, fix_stats).
pub fn auto_fix_poc(content: &str) -> (String, FixStats) {
    let mut fixed = content.to_string();
    let mut stats = FixStats::default();

    // Fix 1: Severity casing
    let sev_re = regex::Regex::new(r"(?m)^(\s*severity:\s*)(\S+)\s*$").unwrap();
    let mut new_content = String::new();
    let mut last_end = 0;
    for cap in sev_re.captures_iter(content) {
        let full_match = cap.get(0).unwrap();
        let prefix = cap.get(1).unwrap().as_str();
        let sev_val = cap.get(2).unwrap().as_str();
        let sev_lower = sev_val.to_lowercase();

        if VALID_SEVERITIES.contains(&sev_lower.as_str()) && sev_val != sev_lower {
            new_content.push_str(&content[last_end..full_match.start()]);
            new_content.push_str(&format!("{}{}", prefix, sev_lower));
            last_end = full_match.end();
            stats.severity_casing += 1;
        }
    }
    if last_end > 0 {
        new_content.push_str(&content[last_end..]);
        fixed = new_content;
    }

    // Fix 2: Empty severity → set to "info"
    let empty_sev_re = regex::Regex::new(r"(?m)^(\s*severity:\s*)$").unwrap();
    let before = fixed.clone();
    fixed = empty_sev_re.replace_all(&fixed, "${1}info").to_string();
    if before != fixed {
        stats.severity_empty += 1;
    }

    // Fix 3: CVE/CNVD/CNNVD ID spaces (CVE 2020-6171 → CVE-2020-6171)
    let id_space_re = regex::Regex::new(r"(?i)(CVE|CNVD|CNNVD)\s+(\d{4}-\d+)").unwrap();
    if id_space_re.is_match(&fixed) {
        let before = fixed.clone();
        fixed = id_space_re.replace_all(&fixed, "$1-$2").to_string();
        if before != fixed {
            stats.id_spaces += 1;
        }
    }

    stats.total_fixed = stats.severity_casing + stats.severity_empty + stats.id_spaces;
    (fixed, stats)
}
