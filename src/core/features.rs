//! POC feature extraction & multi-factor similarity scoring.
//!
//! Extracted from the advanced dedup pipeline so every stage can reuse
//! the same parsing, scoring, and comparison logic.

use crate::core::{hash, yaml};
use regex::Regex;
use serde_yaml::Value;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

// ---------------------------------------------------------------------------
// regex helpers
// ---------------------------------------------------------------------------

static HTTP_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(GET|POST|PUT|DELETE|HEAD|OPTIONS|PATCH)\s+([^\s]+)\s+HTTP/\d").unwrap());

// ---------------------------------------------------------------------------
// PocFeatures
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PocFeatures {
    pub file_path: PathBuf,
    pub id: Option<String>,
    pub cve_id: Option<String>,
    pub cnvd_id: Option<String>,
    pub name: Option<String>,
    pub severity: Option<String>,
    pub tags: Vec<String>,
    pub url_method_pairs: Vec<(String, String)>,
    pub matcher_keywords: HashSet<String>,
    pub matcher_status_codes: HashSet<String>,
    pub matcher_types: HashSet<String>,
    pub has_requests: bool,
    pub has_http: bool,
    pub has_tcp: bool,
    pub has_dns: bool,
    pub has_matchers: bool,
    pub has_extractors: bool,
    pub has_author: bool,
    pub has_description: bool,
    pub has_tags: bool,
    pub has_reference: bool,
    pub has_classification: bool,
    pub has_remediation: bool,
    pub severity_casing_issue: bool,
    pub request_count: usize,
    pub content_hash: String,
    pub file_size: u64,
    pub raw_content: Vec<u8>,
    pub valid: bool,
    pub error_msg: Option<String>,
}

impl PocFeatures {
    pub fn empty(path: PathBuf) -> Self {
        PocFeatures {
            file_path: path,
            id: None,
            cve_id: None,
            cnvd_id: None,
            name: None,
            severity: None,
            tags: Vec::new(),
            url_method_pairs: Vec::new(),
            matcher_keywords: HashSet::new(),
            matcher_status_codes: HashSet::new(),
            matcher_types: HashSet::new(),
            has_requests: false,
            has_http: false,
            has_tcp: false,
            has_dns: false,
            has_matchers: false,
            has_extractors: false,
            has_author: false,
            has_description: false,
            has_tags: false,
            has_reference: false,
            has_classification: false,
            has_remediation: false,
            severity_casing_issue: false,
            request_count: 0,
            content_hash: String::new(),
            file_size: 0,
            raw_content: Vec::new(),
            valid: false,
            error_msg: None,
        }
    }

    /// Quality score for choosing the "best" POC when duplicates are found.
    /// Higher = better quality. 18 factors, 0-80 scale.
    pub fn quality_score(&self) -> i32 {
        let mut s = 0;

        // Basic structure (0-7)
        if self.id.is_some() { s += 3; }
        if self.name.is_some() { s += 2; }
        if self.severity.is_some() { s += 2; }

        // Severity level (0-8)
        if let Some(ref sev) = self.severity {
            match sev.as_str() {
                "critical" => s += 8,
                "high" => s += 6,
                "medium" => s += 4,
                "low" => s += 2,
                "info" => s += 1,
                _ => {}
            }
        }

        // Protocol support (0-10)
        if self.has_http {
            s += if self.has_matchers { 6 } else { 4 };
        } else if self.has_requests {
            s += if self.has_matchers { 5 } else { 3 };
        }
        if self.has_tcp { s += 3; }
        if self.has_dns { s += 3; }

        // Metadata richness (0-16)
        for &field in &[self.has_author, self.has_description, self.has_tags,
                         self.has_reference, self.has_classification, self.has_remediation] {
            if field { s += 2; }
        }

        // Detection capability (0-15)
        if self.has_matchers { s += 5; }
        if self.has_extractors { s += 4; }
        s += self.url_method_pairs.len().min(6) as i32;

        // Vulnerability association (0-10)
        if self.cve_id.is_some() { s += 6; }
        if self.cnvd_id.is_some() { s += 4; }

        // Format normalization (0-6)
        if self.has_http && !self.has_requests { s += 3; }
        if !self.severity_casing_issue { s += 1; }
        s += 2; // no deprecated network

        // File size appropriateness (0-5)
        let fsize = self.file_size;
        if (500..=10000).contains(&fsize) { s += 5; }
        else if (200..=20000).contains(&fsize) { s += 3; }
        else if fsize < 200 { s += 1; }
        else { s += 2; }

        // Multi-protocol bonus (0-3)
        let proto_count = (self.has_http || self.has_requests) as i32
            + self.has_tcp as i32
            + self.has_dns as i32;
        if proto_count >= 2 { s += 3; }

        s
    }
}

// ============================================================================
// Feature Extraction
// ============================================================================

/// Parse a YAML file and extract all POC features.
pub fn extract(file_path: &Path) -> PocFeatures {
    let mut features = PocFeatures::empty(file_path.to_path_buf());

    // Read file
    let mut buf = Vec::new();
    let file = match fs::File::open(file_path) {
        Ok(mut f) => {
            if f.read_to_end(&mut buf).is_err() {
                features.error_msg = Some("read failed".into());
                return features;
            }
            f
        }
        Err(e) => {
            features.error_msg = Some(format!("open failed: {}", e));
            return features;
        }
    };
    drop(file);

    features.file_size = buf.len() as u64;
    features.raw_content = buf.clone();
    features.content_hash = hash::hash_bytes(&buf);

    let yaml_text = match std::str::from_utf8(&buf) {
        Ok(s) => s.to_string(),
        Err(_) => {
            features.error_msg = Some("invalid utf8".into());
            return features;
        }
    };

    let yaml: Value = match serde_yaml::from_str(&yaml_text) {
        Ok(v) => v,
        Err(e) => {
            features.error_msg = Some(format!("yaml parse: {}", e));
            return features;
        }
    };

    // Extract ID
    features.id = yaml.get("id").and_then(|v| v.as_str()).map(|s| s.to_string());

    // Extract info block — use shared yaml helpers
    if let Some(info_val) = yaml.get("info") {
        if let Some(info) = info_val.as_mapping() {
            features.name = info.get("name").and_then(|v| v.as_str()).map(|s| s.to_string());
            features.severity = info.get("severity").and_then(|v| v.as_str())
                .map(|s| s.to_string().to_lowercase());
            features.has_author = info.get("author").is_some();
            features.has_description = info.get("description").is_some();
            features.has_tags = info.get("tags").is_some();
            features.has_reference = info.get("reference").is_some();
            features.has_remediation = info.get("remediation").is_some();

            // Extract tags list for classification
            if let Some(tags_val) = info.get("tags") {
                match tags_val {
                    Value::String(s) => {
                        features.tags = s.split(',').map(|t| t.trim().to_lowercase()).collect();
                    }
                    Value::Sequence(seq) => {
                        features.tags = seq.iter()
                            .filter_map(|v| v.as_str())
                            .map(|s| s.trim().to_lowercase())
                            .collect();
                    }
                    _ => {}
                }
            }

            // CVE / CNVD via shared helpers
            features.cve_id = yaml::extract_cve_from_info(info_val);
            features.cnvd_id = yaml::extract_cnvd_from_info(info_val);
            features.has_classification = info.get("classification").is_some();
        }
    }

    // Fallback CVE/CNVD from ID field and full content (use shared helpers)
    if features.cve_id.is_none() {
        if let Some(ref id) = features.id {
            features.cve_id = yaml::extract_cve(id);
        }
    }
    if features.cnvd_id.is_none() {
        if let Some(ref id) = features.id {
            features.cnvd_id = yaml::extract_cnvd(id);
        }
    }
    if features.cve_id.is_none() {
        features.cve_id = yaml::extract_cve(&yaml_text);
    }
    if features.cnvd_id.is_none() {
        features.cnvd_id = yaml::extract_cnvd(&yaml_text);
    }

    // Check structure type
    features.has_requests = yaml.get("requests").is_some();
    features.has_http = yaml.get("http").is_some();
    features.has_tcp = yaml.get("tcp").is_some();
    features.has_dns = yaml.get("dns").is_some();

    // Check severity casing
    features.severity_casing_issue = features.severity.as_ref()
        .map(|s| yaml::has_severity_casing_issue(s))
        .unwrap_or(false);

    // Extract requests from both "requests" and "http" fields
    let request_containers = [yaml.get("requests"), yaml.get("http")];
    let mut url_method_set: HashSet<(String, String)> = HashSet::new();

    for container in request_containers.iter() {
        if let Some(seq) = container.and_then(|v| v.as_sequence()) {
            features.request_count += seq.len();
            for req in seq {
                // Method + path
                let method = req.get("method")
                    .and_then(|v| v.as_str())
                    .unwrap_or("GET")
                    .to_uppercase();

                if let Some(paths) = req.get("path") {
                    let path_list: Vec<&str> = match paths {
                        Value::String(s) => vec![s],
                        Value::Sequence(seq) => seq.iter().filter_map(|v| v.as_str()).collect(),
                        _ => vec![],
                    };
                    for p in path_list {
                        let clean = p.trim().trim_matches(|c| c == '"' || c == '\'');
                        if !clean.is_empty() && !clean.starts_with("{{") {
                            url_method_set.insert((method.clone(), normalize_url(clean)));
                        }
                    }
                }

                // Raw requests
                if let Some(raw) = req.get("raw") {
                    let raw_list: Vec<&str> = match raw {
                        Value::String(s) => vec![s],
                        Value::Sequence(seq) => seq.iter().filter_map(|v| v.as_str()).collect(),
                        _ => vec![],
                    };
                    for raw_str in raw_list {
                        for cap in HTTP_RE.captures_iter(raw_str) {
                            let m = cap.get(1).unwrap().as_str().to_uppercase();
                            let url = cap.get(2).unwrap().as_str();
                            let clean = url.trim().trim_matches(|c| c == '"' || c == '\'');
                            if !clean.is_empty() && !clean.starts_with("{{") {
                                url_method_set.insert((m, normalize_url(clean)));
                            }
                        }
                    }
                }

                // Matchers
                if let Some(matchers) = req.get("matchers").and_then(|v| v.as_sequence()) {
                    features.has_matchers = true;
                    for m in matchers {
                        if let Some(t) = m.get("type").and_then(|v| v.as_str()) {
                            features.matcher_types.insert(t.to_string());
                        }
                        if let Some(status) = m.get("status") {
                            match status {
                                Value::Number(n) => {
                                    features.matcher_status_codes.insert(n.to_string());
                                }
                                Value::Sequence(seq) => {
                                    for s in seq {
                                        if let Some(n) = s.as_i64() {
                                            features.matcher_status_codes.insert(n.to_string());
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                        if let Some(words) = m.get("words") {
                            match words {
                                Value::String(s) => {
                                    features.matcher_keywords.insert(s.clone());
                                }
                                Value::Sequence(seq) => {
                                    for w in seq {
                                        if let Some(s) = w.as_str() {
                                            features.matcher_keywords.insert(s.to_string());
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }

                // Extractors
                if req.get("extractors").is_some() {
                    features.has_extractors = true;
                }
            }
        }
    }

    features.url_method_pairs = url_method_set.into_iter().collect();
    features.valid = true;
    features
}

/// Build CVE and URL indexes from a list of feature indices.
pub fn build_indexes(
    features: &[PocFeatures],
    indices: &[usize],
) -> (HashMap<String, Vec<usize>>, HashMap<String, Vec<usize>>) {
    let mut cve_index: HashMap<String, Vec<usize>> = HashMap::new();
    let mut url_index: HashMap<String, Vec<usize>> = HashMap::new();

    for &idx in indices {
        let f = &features[idx];
        if let Some(ref cve) = f.cve_id {
            cve_index.entry(cve.clone()).or_default().push(idx);
        }
        for (method, url) in &f.url_method_pairs {
            url_index.entry(format!("{} {}", method, url)).or_default().push(idx);
        }
    }
    (cve_index, url_index)
}

// ============================================================================
// URL Normalization
// ============================================================================

/// Normalize a URL for comparison: lowercased, query params sorted.
pub fn normalize_url(url: &str) -> String {
    let url = url.trim();
    if let Some(pos) = url.find('?') {
        let path = &url[..pos];
        let query = &url[pos + 1..];
        let mut params: Vec<&str> = query.split('&')
            .map(|p| if p.contains('=') { p.split('=').next().unwrap() } else { p })
            .collect();
        params.sort();
        format!("{}?{}", path, params.join("&"))
    } else {
        url.to_string()
    }.to_lowercase()
}

// ============================================================================
// Multi-Factor Similarity Scoring
// ============================================================================

#[derive(Debug)]
pub struct MatchDetails {
    pub id_match: bool,
    pub cve_match: bool,
    pub cnvd_match: bool,
    pub url_full_match: bool,
    pub url_partial_match: bool,
    pub matcher_similar: bool,
    pub name_similarity: f64,
    pub matched_urls: Vec<(String, String)>,
    pub score: i32,
}

/// Calculate similarity score between two POC features.
/// Returns (score, details). Score ≥ 70 is considered duplicate.
pub fn calculate_similarity(f1: &PocFeatures, f2: &PocFeatures) -> (i32, MatchDetails) {
    let mut score = 0i32;
    let mut details = MatchDetails {
        id_match: false,
        cve_match: false,
        cnvd_match: false,
        url_full_match: false,
        url_partial_match: false,
        matcher_similar: false,
        name_similarity: 0.0,
        matched_urls: Vec::new(),
        score: 0,
    };

    // 1. ID match: 100 points → immediate duplicate
    if let (Some(id1), Some(id2)) = (&f1.id, &f2.id) {
        if id1 == id2 {
            score = 100;
            details.id_match = true;
            details.score = score;
            return (score, details);
        }
    }

    // 2. CVE/CNVD match: 30 points each (alone does NOT cross the default
    //    70-point threshold — same-CVE templates targeting different products
    //    or endpoints must NOT be treated as duplicates).
    if let (Some(cve1), Some(cve2)) = (&f1.cve_id, &f2.cve_id) {
        if cve1 == cve2 {
            score += 30;
            details.cve_match = true;
        }
    }
    if let (Some(cnvd1), Some(cnvd2)) = (&f1.cnvd_id, &f2.cnvd_id) {
        if cnvd1 == cnvd2 {
            score += 30;
            details.cnvd_match = true;
        }
    }

    // 3. URL+Method matching
    let pairs1: HashSet<_> = f1.url_method_pairs.iter().cloned().collect();
    let pairs2: HashSet<_> = f2.url_method_pairs.iter().cloned().collect();

    if !pairs1.is_empty() && !pairs2.is_empty() {
        let common: HashSet<_> = pairs1.intersection(&pairs2).collect();
        if !common.is_empty() {
            score += 60;
            details.url_full_match = true;
            for (m, u) in common {
                details.matched_urls.push((format!("{} {}", m, u), format!("{} {}", m, u)));
            }
        } else {
            let urls1: HashSet<_> = pairs1.iter().map(|(_, u)| u).collect();
            let urls2: HashSet<_> = pairs2.iter().map(|(_, u)| u).collect();
            if !urls1.is_empty() && !urls1.is_disjoint(&urls2) {
                score += 40;
                details.url_partial_match = true;
            }
        }
    }

    // 4. Matcher similarity: 10-30 points
    if !f1.matcher_status_codes.is_empty() && !f2.matcher_status_codes.is_empty() {
        if !f1.matcher_status_codes.is_disjoint(&f2.matcher_status_codes) {
            score += 10;
        }
    }
    if !f1.matcher_keywords.is_empty() && !f2.matcher_keywords.is_empty() {
        let common: HashSet<_> = f1.matcher_keywords.intersection(&f2.matcher_keywords).collect();
        if !common.is_empty() {
            let ratio = common.len() as f64
                / f1.matcher_keywords.len().max(f2.matcher_keywords.len()) as f64;
            score += (20.0 * ratio) as i32;
            if ratio > 0.3 {
                details.matcher_similar = true;
            }
        }
    }
    if !f1.matcher_types.is_empty() && !f2.matcher_types.is_empty() {
        if !f1.matcher_types.is_disjoint(&f2.matcher_types) {
            score += 5;
        }
    }

    // 5. Name similarity: 10-20 points
    if let (Some(n1), Some(n2)) = (&f1.name, &f2.name) {
        let sim = simple_name_similarity(&n1.to_lowercase(), &n2.to_lowercase());
        details.name_similarity = sim;
        if sim > 0.8 { score += 20; }
        else if sim > 0.6 { score += 10; }
    }

    details.score = score;
    (score, details)
}

// ============================================================================
// Name Similarity
// ============================================================================

/// Simple name similarity based on word overlap (0.6) and LCS ratio (0.4).
fn simple_name_similarity(a: &str, b: &str) -> f64 {
    if a == b { return 1.0; }

    let words_a: HashSet<&str> = a.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty()).collect();
    let words_b: HashSet<&str> = b.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty()).collect();

    if words_a.is_empty() || words_b.is_empty() { return 0.0; }

    let intersection = words_a.intersection(&words_b).count();
    let union = words_a.union(&words_b).count();
    let word_sim = intersection as f64 / union as f64;

    let char_sim = {
        let a_chars: Vec<char> = a.chars().collect();
        let b_chars: Vec<char> = b.chars().collect();
        if a_chars.is_empty() || b_chars.is_empty() { 0.0 }
        else {
            let lcs = lcs_length(&a_chars, &b_chars);
            lcs as f64 / a_chars.len().max(b_chars.len()) as f64
        }
    };

    word_sim * 0.6 + char_sim * 0.4
}

fn lcs_length(a: &[char], b: &[char]) -> usize {
    let n = a.len();
    let m = b.len();
    let mut prev = vec![0usize; m + 1];
    let mut curr = vec![0usize; m + 1];
    for i in 1..=n {
        for j in 1..=m {
            curr[j] = if a[i - 1] == b[j - 1] {
                prev[j - 1] + 1
            } else {
                prev[j].max(curr[j - 1])
            };
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[m]
}
