use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use serde_yaml::Value;
use clap::Parser;
use rayon::prelude::*;
use serde::Serialize;

#[derive(Parser, Debug)]
#[command(name = "8_dedup_high_quality", about = "Select high-quality POCs and rebuild directory structure")]
struct Args {
    /// Minimum score to include a POC
    #[arg(short, long, default_value_t = 5)]
    min_score: i32,

    /// Perform a dry run (no file copy), just print stats
    #[arg(long, default_value_t = false)]
    dry_run: bool,

    /// Require these top-level fields to exist (comma-separated)
    #[arg(long, value_delimiter = ',', default_value = "id,info,requests")]
    require_fields: Vec<String>,

    /// Allowed severities (comma-separated), empty means any
    #[arg(long, value_delimiter = ',', default_value = "critical,high,medium")]
    allow_severities: Vec<String>,

    /// Minimum number of requests
    #[arg(long, default_value_t = 1)]
    min_requests: usize,

    /// Require presence of matchers in at least one request
    #[arg(long, default_value_t = false)]
    require_matchers: bool,

    /// Source whitelist substring match (e.g., repo path/tag)
    #[arg(long, value_delimiter = ',', default_value = "")]
    source_whitelist: Vec<String>,
}

fn score_yaml(yaml: &Value) -> i32 {
    let mut score = 0;

    // Basic presence of key metadata
    if yaml.get("id").is_some() { score += 2; }
    if yaml.get("info").is_some() { score += 2; }

    if let Some(info) = yaml.get("info") {
        if info.get("name").is_some() { score += 1; }
        if info.get("severity").is_some() { score += 2; }
        if let Some(sev) = info.get("severity").and_then(|v| v.as_str()) {
            match sev {
                "high" | "critical" => score += 3,
                "medium" => score += 2,
                "low" => score += 1,
                _ => {}
            }
        }
        if info.get("description").is_some() { score += 1; }
        if info.get("tags").is_some() { score += 1; }
        if info.get("references").is_some() { score += 1; }
    }

    // Requests richness
    if let Some(requests) = yaml.get("requests") {
        if let Some(arr) = requests.as_sequence() {
            let count = arr.len();
            if count >= 1 { score += 2; }
            if count >= 2 { score += 1; }
            if count >= 4 { score += 1; }

            // Matchers/extractors presence
            for req in arr {
                if req.get("matchers").is_some() { score += 2; }
                if req.get("extractors").is_some() { score += 1; }
                if req.get("path").is_some() { score += 1; }
                if req.get("method").is_some() { score += 1; }
            }
        }
    }

    // Additional signals commonly used
    if yaml.get("variables").is_some() { score += 1; }
    if yaml.get("http").is_some() { score += 1; }
    if yaml.get("tcp").is_some() { score += 1; }
    if yaml.get("headless").is_some() { score += 1; }
    if yaml.get("flows").is_some() { score += 1; }

    score
}

#[derive(Serialize)]
struct ReportItem {
    path: String,
    score: i32,
    selected: bool,
    reasons: Vec<String>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let src_root = Path::new("poc_all");
    let dst_root = Path::new("poc_high_quality");

    // Cleanup destination each run
    if dst_root.exists() {
        fs::remove_dir_all(dst_root).context("Failed to remove poc_high_quality directory")?;
    }
    fs::create_dir_all(dst_root).context("Failed to create poc_high_quality directory")?;

    // Collect all files under poc_all
    let files: Vec<PathBuf> = WalkDir::new(src_root)
        .into_iter()
        .filter_map(|e| e.ok())
        .map(|e| e.path().to_path_buf())
        .filter(|p| p.is_file())
        .collect();

    // Deduplicate by content hash and copy while preserving directory structure
    let mut seen: HashSet<String> = HashSet::new();
    let mut considered = 0usize;
    let mut selected = 0usize;
    let mut report: Vec<ReportItem> = Vec::new();

    // Parallel evaluation of files
    let evaluated: Vec<(PathBuf, i32, Vec<String>, Vec<u8>, String)> = files
        .par_iter()
        .map(|file| {
            let mut reasons: Vec<String> = Vec::new();
            let mut buf = Vec::new();
            let mut score = 0;
            let mut hash = String::new();
            match fs::File::open(file) {
                Ok(mut f) => {
                    if let Err(_) = f.read_to_end(&mut buf) {
                        reasons.push("read failed".to_string());
                    } else {
                        hash = format!("{:x}", Sha256::digest(&buf));
                        match serde_yaml::from_slice::<Value>(&buf) {
                            Ok(v) => {
                                score = score_yaml(&v);
                                for req in &args.require_fields {
                                    if !v.get(req).is_some() {
                                        reasons.push(format!("missing field: {}", req));
                                    }
                                }
                                if let Some(info) = v.get("info") {
                                    if let Some(sev) = info.get("severity").and_then(|s| s.as_str()) {
                                        if !args.allow_severities.is_empty()
                                            && !args.allow_severities.iter().any(|a| a.eq_ignore_ascii_case(sev))
                                        {
                                            reasons.push(format!("severity not allowed: {}", sev));
                                        }
                                    } else {
                                        reasons.push("missing severity".to_string());
                                    }
                                } else {
                                    reasons.push("missing info".to_string());
                                }
                                let mut req_count = 0usize;
                                let mut has_matchers = false;
                                if let Some(requests) = v.get("requests").and_then(|r| r.as_sequence()) {
                                    req_count = requests.len();
                                    for req in requests {
                                        if req.get("matchers").is_some() { has_matchers = true; }
                                    }
                                } else {
                                    reasons.push("missing requests".to_string());
                                }
                                if req_count < args.min_requests {
                                    reasons.push(format!("requests too few: {} < {}", req_count, args.min_requests));
                                }
                                if args.require_matchers && !has_matchers {
                                    reasons.push("no matchers".to_string());
                                }
                                if !args.source_whitelist.is_empty() {
                                    let p = file.display().to_string();
                                    if !args.source_whitelist.iter().any(|s| !s.is_empty() && p.contains(s)) {
                                        reasons.push("source not whitelisted".to_string());
                                    }
                                }
                            }
                            Err(_) => {
                                reasons.push("invalid yaml".to_string());
                            }
                        }
                    }
                }
                Err(_) => reasons.push("open failed".to_string()),
            }
            (file.clone(), score, reasons, buf, hash)
        })
        .collect();

    // Deterministic copy and build report
    for (file, score, reasons, buf, hash) in evaluated {
        considered += 1;
        let mut selected_flag = false;
        let missing_required = reasons.iter().any(|r| r.starts_with("missing field"));
        if score >= args.min_score && !missing_required {
            if !seen.contains(&hash) {
                seen.insert(hash);
                selected += 1;
                selected_flag = true;
                if !args.dry_run {
                    let rel = file.strip_prefix(src_root).unwrap();
                    let dst_path = dst_root.join(rel);
                    if let Some(parent) = dst_path.parent() {
                        fs::create_dir_all(parent).with_context(|| format!("Create dir failed: {}", parent.display()))?;
                    }
                    fs::write(&dst_path, &buf)
                        .with_context(|| format!("Write file failed: {}", dst_path.display()))?;
                }
            }
        }
        report.push(ReportItem {
            path: file.display().to_string(),
            score,
            selected: selected_flag,
            reasons,
        });
    }

    // Write report JSON
    let report_path = Path::new("poc_high_quality_report.json");
    let summary = serde_json::json!({
        "evaluated": considered,
        "selected": selected,
        "unique": seen.len(),
        "min_score": args.min_score,
    });
    let output = serde_json::json!({
        "summary": summary,
        "items": report,
    });
    fs::write(report_path, serde_json::to_vec_pretty(&output)?).context("write report failed")?;

    println!(
        "Evaluated {} files, selected {}, unique copied {} (min_score={})",
        considered,
        selected,
        seen.len(),
        args.min_score
    );
    Ok(())
}

