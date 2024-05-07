//! Structured JSON index generation for the POC archive.
//!
//! Generates `poc_index.json` with metadata for every POC file, enabling
//! downstream tools to query and filter without parsing all YAML files.

use crate::core::features;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// A single entry in the POC index.
#[derive(serde::Serialize, Debug, Clone)]
pub struct PocIndexEntry {
    pub file: String,
    pub id: Option<String>,
    pub name: Option<String>,
    pub cve: Option<String>,
    pub cnvd: Option<String>,
    pub severity: Option<String>,
    pub tags: Vec<String>,
    pub protocol: String,
    pub category: String,
    pub hash: String,
    pub score: i32,
    pub file_size: u64,
}

/// Generate a structured JSON index from a POC directory.
///
/// Walks `poc_dir`, extracts features from each YAML file, and writes
/// `poc_index.json` to `output_dir`.
pub fn generate_index(
    poc_dir: &str,
    output_dir: &str,
    _cmap: &HashMap<&str, Vec<&str>>,
) -> anyhow::Result<(Vec<PocIndexEntry>, HashMap<String, usize>)> {
    let mut entries = Vec::new();
    let mut category_counts: HashMap<String, usize> = HashMap::new();

    let yaml_files: Vec<PathBuf> = WalkDir::new(poc_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file()
                && e.path()
                    .extension()
                    .map_or(false, |ext| ext == "yaml" || ext == "yml")
        })
        .map(|e| e.path().to_path_buf())
        .collect();

    for path in &yaml_files {
        let features = features::extract(path);
        if !features.valid {
            continue;
        }

        let rel_path = path
            .strip_prefix(poc_dir)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        // Determine category from path
        let category = Path::new(&rel_path)
            .parent()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "other".to_string());

        // Protocol
        let protocol = if features.has_http {
            "http".to_string()
        } else if features.has_requests {
            "requests".to_string()
        } else if features.has_tcp {
            "tcp".to_string()
        } else if features.has_dns {
            "dns".to_string()
        } else {
            "unknown".to_string()
        };

        let score = features.quality_score();

        let entry = PocIndexEntry {
            file: rel_path.clone(),
            id: features.id.clone(),
            name: features.name.clone(),
            cve: features.cve_id.clone(),
            cnvd: features.cnvd_id.clone(),
            severity: features.severity.clone(),
            tags: features.tags.clone(),
            protocol,
            category: category.clone(),
            hash: features.content_hash.clone(),
            score,
            file_size: features.file_size,
        };

        *category_counts.entry(category).or_insert(0) += 1;
        entries.push(entry);
    }

    // Sort by score descending
    entries.sort_by(|a, b| b.score.cmp(&a.score));

    // Write JSON
    let json_path = Path::new(output_dir).join("poc_index.json");
    let json = serde_json::to_string_pretty(&entries)?;
    fs::write(&json_path, json)?;
    println!("  poc_index.json 已生成: {} 条记录", entries.len());

    Ok((entries, category_counts))
}

/// Also write a plain poc.txt for backward compatibility.
pub fn write_poc_txt(output_dir: &str, entries: &[PocIndexEntry]) -> anyhow::Result<()> {
    use std::io::{BufWriter, Write};
    let f = fs::File::create(Path::new(output_dir).join("poc.txt"))?;
    let mut w = BufWriter::new(f);
    for e in entries {
        writeln!(w, "{}", e.file)?;
    }
    Ok(())
}

/// Generate summary statistics JSON.
pub fn write_summary_json(
    output_dir: &str,
    total_files: usize,
    category_counts: &HashMap<String, usize>,
) -> anyhow::Result<()> {
    let mut summary = serde_json::Map::new();
    summary.insert(
        "total_valid".into(),
        Value::Number(total_files.into()),
    );

    let mut cats = serde_json::Map::new();
    let mut sorted: Vec<_> = category_counts.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(a.1));
    for (k, v) in &sorted {
        cats.insert(k.to_string(), Value::Number((**v).into()));
    }
    summary.insert("categories".into(), Value::Object(cats));

    summary.insert(
        "generated_at".into(),
        Value::String(chrono::Utc::now().to_rfc3339()),
    );

    let path = Path::new(output_dir).join("poc_summary.json");
    fs::write(&path, serde_json::to_string_pretty(&Value::Object(summary))?)?;
    Ok(())
}
