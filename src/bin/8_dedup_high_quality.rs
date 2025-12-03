use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use serde_yaml::Value;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "8_dedup_high_quality", about = "Select high-quality POCs and rebuild directory structure")]
struct Args {
    /// Minimum score to include a POC
    #[arg(short, long, default_value_t = 5)]
    min_score: i32,

    /// Perform a dry run (no file copy), just print stats
    #[arg(long, default_value_t = false)]
    dry_run: bool,
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
    let mut files: Vec<PathBuf> = Vec::new();
    for entry in WalkDir::new(src_root).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        if p.is_file() {
            files.push(p.to_path_buf());
        }
    }

    // Deduplicate by content hash and copy while preserving directory structure
    let mut seen: HashSet<String> = HashSet::new();
    let mut considered = 0usize;
    let mut selected = 0usize;

    // Process sequentially to preserve deterministic behavior
    for file in files {
        let mut f = fs::File::open(&file).with_context(|| format!("Open file failed: {}", file.display()))?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).with_context(|| format!("Read file failed: {}", file.display()))?;
        considered += 1;

        // Try to parse YAML and score
        let score = match serde_yaml::from_slice::<Value>(&buf) {
            Ok(v) => score_yaml(&v),
            Err(_) => 0,
        };

        if score < args.min_score {
            continue; // skip low-score files
        }

        // Dedup by content
        let hash = format!("{:x}", Sha256::digest(&buf));
        if seen.contains(&hash) {
            continue;
        }
        seen.insert(hash);

        selected += 1;

        if args.dry_run {
            continue;
        }

        // Compute destination path preserving relative structure
        let rel = file.strip_prefix(src_root).unwrap();
        let dst_path = dst_root.join(rel);
        if let Some(parent) = dst_path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("Create dir failed: {}", parent.display()))?;
        }
        fs::write(&dst_path, &buf)
            .with_context(|| format!("Write file failed: {}", dst_path.display()))?;
    }

    println!(
        "Evaluated {} files, selected {}, unique copied {} (min_score={})",
        considered,
        selected,
        seen.len(),
        args.min_score
    );
    Ok(())
}

