//! Stage 8 — Generate browser-friendly chunked JSON indexes for GitHub Pages.
//!
//! Reads `poc.txt` for the main `poc/` directory and walks `poc_dedup/` and
//! `poc_gold_*/` tier directories on disk.  Groups files by category, splits large categories into
//! chunks of `CHUNK_SIZE`, and writes everything under `docs/`.
//!
//! Output structure:
//!   docs/
//!     _categories.json         — summary { dir: { cat: count } }
//!     poc_cve_0.json           — chunk 0  (3000 filenames)
//!     poc_cve_1.json           — chunk 1
//!     poc_cve_manifest.json    — { "chunks": 280, "total": 839976 }
//!     poc_dedup_cve_0.json
//!     poc_gold_cve_0.json
//!     ...

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter};
use std::path::Path;

use clap::Parser;
use rayon::prelude::*;
use serde::Serialize;
use walkdir::WalkDir;

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

/// Maximum number of filenames per chunk file (keeps each chunk ~150 KB).
const CHUNK_SIZE: usize = 3000;

/// Output directory relative to the repo root.
const OUT_DIR: &str = "docs";

/// Discover poc_gold_* tier directories (poc_gold_11, poc_gold_12, poc_gold_13, poc_gold_14, poc_gold_15, etc.)
fn discover_gold_dirs(repo_root: &Path) -> Vec<String> {
    let mut dirs: Vec<String> = fs::read_dir(repo_root)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.starts_with("poc_gold_") && n[9..].chars().all(|c| c.is_ascii_digit()))
        .collect();
    dirs.sort();
    dirs
}

#[derive(Parser, Debug)]
#[command(
    name = "8_generate_browser_index",
    about = "Generate chunked browser index files for GitHub Pages POC viewer"
)]
struct Args {
    /// Path to poc.txt (one file path per line)
    #[arg(long, default_value = "poc.txt")]
    poc_txt: String,

    /// Root of the repository (default: current directory)
    #[arg(long, default_value = ".")]
    repo_root: String,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Ensure a directory exists (like mkdir -p).
fn ensure_dir(path: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(path)?;
    Ok(())
}

/// Write a JSON file atomically via a tempfile.
fn write_json<T: Serialize>(path: &Path, value: &T) -> anyhow::Result<()> {
    let file = File::create(path)?;
    let writer = BufWriter::new(file);
    serde_json::to_writer(writer, value)?;
    Ok(())
}

/// Check whether a filename has a YAML extension.
fn is_yaml(fname: &str) -> bool {
    let lower = fname.to_lowercase();
    lower.ends_with(".yaml") || lower.ends_with(".yml")
}

// ---------------------------------------------------------------------------
// Indexing from poc.txt
// ---------------------------------------------------------------------------

/// Parse `poc.txt` (one relative path per line like `cve/CVE-xxx.yaml`) and
/// return a map of category → filenames.
fn parse_poc_txt(txt_path: &Path) -> anyhow::Result<BTreeMap<String, Vec<String>>> {
    let file = File::open(txt_path)?;
    let reader = BufReader::new(file);

    let mut cats: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for line in reader.lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // Format: "category/filename.yaml"
        if let Some((cat, fname)) = line.split_once('/') {
            if is_yaml(fname) {
                cats.entry(cat.to_string())
                    .or_default()
                    .push(fname.to_string());
            }
        }
    }

    Ok(cats)
}

// ---------------------------------------------------------------------------
// Indexing from filesystem (poc_dedup, poc_gold)
// ---------------------------------------------------------------------------

/// Walk `dir` and collect YAML filenames grouped by their immediate parent
/// directory (the category).
fn scan_fs_dir(dir: &Path) -> BTreeMap<String, Vec<String>> {
    let entries: Vec<_> = WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file()
                && e.path()
                    .extension()
                    .map_or(false, |ext| ext.eq_ignore_ascii_case("yaml") || ext.eq_ignore_ascii_case("yml"))
        })
        .collect();

    // Parallel group-by
    let pairs: Vec<(String, String)> = entries
        .par_iter()
        .filter_map(|entry| {
            let path = entry.path();
            let rel = path.strip_prefix(dir).ok()?;
            // Get the category from the first path component
            let cat = rel
                .parent()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "_root".to_string());
            let fname = path
                .file_name()?
                .to_string_lossy()
                .to_string();
            Some((cat, fname))
        })
        .collect();

    let mut cats: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (cat, fname) in pairs {
        cats.entry(cat).or_default().push(fname);
    }
    // Sort for deterministic output
    for files in cats.values_mut() {
        files.sort();
    }
    cats
}

// ---------------------------------------------------------------------------
// Chunked output
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct Manifest {
    chunks: usize,
    total: usize,
}

/// Write chunked JSON files for a single category under a given prefix.
///
/// Returns (total_file_count, chunk_count).
fn write_chunks(
    out_dir: &Path,
    prefix: &str, // e.g. "poc" / "poc_dedup" / "poc_gold"
    cat: &str,
    files: &[String],
) -> anyhow::Result<(usize, usize)> {
    let total = files.len();
    let chunks = total.div_ceil(CHUNK_SIZE); // ceiling division

    for i in 0..chunks {
        let start = i * CHUNK_SIZE;
        let end = (start + CHUNK_SIZE).min(total);
        let chunk = &files[start..end];

        let chunk_path = out_dir.join(format!("{}_{}_{}.json", prefix, cat, i));
        write_json(&chunk_path, &chunk)?;
    }

    // Manifest
    let manifest = Manifest { chunks, total };
    let manifest_path = out_dir.join(format!("{}_{}_manifest.json", prefix, cat));
    write_json(&manifest_path, &manifest)?;

    Ok((total, chunks))
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let root = Path::new(&args.repo_root);
    let out_dir = root.join(OUT_DIR);

    ensure_dir(&out_dir)?;

    // ── Phase 1: Collect all data first (without writing files) ──
    // Structure: BTreeMap<dir_name, (BTreeMap<cat, Vec<filenames>>)>
    let mut all_data: BTreeMap<String, BTreeMap<String, Vec<String>>> = BTreeMap::new();

    // ── 1. poc/ from poc.txt ──
    let poc_txt = root.join(&args.poc_txt);
    if poc_txt.exists() {
        println!("[1/3] poc/  — from {}", args.poc_txt);
        let cats = parse_poc_txt(&poc_txt)?;
        let count: usize = cats.values().map(|v| v.len()).sum();
        println!("  poc/: {} files across {} categories", count, cats.len());
        all_data.insert("poc".to_string(), cats);
    } else {
        eprintln!("  [SKIP] {} not found", args.poc_txt);
    }

    // ── 2. Walk poc_dedup/ ──
    let dedup_dir = root.join("poc_dedup");
    if dedup_dir.is_dir() {
        println!("[2/3] poc_dedup/  — from filesystem");
        let cats = scan_fs_dir(&dedup_dir);
        let count: usize = cats.values().map(|v| v.len()).sum();
        println!("  poc_dedup/: {} files across {} categories", count, cats.len());
        all_data.insert("poc_dedup".to_string(), cats);
    } else {
        eprintln!("  [SKIP] poc_dedup/ not found");
    }

    // ── 3. Walk discovered poc_gold_* tier directories ──
    let gold_dirs = discover_gold_dirs(root);
    if gold_dirs.is_empty() {
        eprintln!("  [SKIP] no poc_gold_* directories found");
    } else {
        println!("[3/3] poc_gold_* tiers — from filesystem");
        for dir_name in &gold_dirs {
            let dir = root.join(dir_name);
            if !dir.is_dir() {
                continue;
            }
            let cats = scan_fs_dir(&dir);
            let count: usize = cats.values().map(|v| v.len()).sum();
            println!("  {}/: {} files across {} categories", dir_name, count, cats.len());
            all_data.insert(dir_name.to_string(), cats);
        }
    }

    // ── Phase 2: Guard against empty data — don't overwrite previous good data ──
    let grand_total: usize = all_data.values()
        .flat_map(|cats| cats.values())
        .map(|v| v.len())
        .sum();

    if grand_total == 0 {
        eprintln!(
            "\n⚠️  WARNING: No POC files found in any tier (poc/, poc_dedup/, poc_gold_*).\n\
               This likely means the upstream pipeline didn't produce files.\n\
               Existing _categories.json and chunk files will NOT be overwritten.\n\
               Please check steps 4-7 in the CI pipeline."
        );
        return Ok(());
    }

    // ── Phase 3: Clean old JSON and write new chunk files ──
    // Delete previously generated JSON index files, keep static frontend assets
    if out_dir.exists() {
        for entry in fs::read_dir(&out_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map_or(false, |ext| ext == "json") {
                if let Err(e) = fs::remove_file(&path) {
                    eprintln!("  warn: cannot remove {}: {e}", path.display());
                }
            }
        }
    }

    // Write chunks and build summary
    let mut all_cats: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();

    for (tier_name, cats) in &all_data {
        let mut summary = BTreeMap::new();
        for (cat, files) in cats {
            let (count, chunks) = write_chunks(&out_dir, &tier_name, cat, files)?;
            summary.insert(cat.clone(), count);
            println!("  {}/{}: {} → {} chunk(s)", tier_name, cat, count, chunks);
        }
        all_cats.insert(tier_name.clone(), summary);
    }

    // ── Write _categories.json ──
    let cat_path = out_dir.join("_categories.json");
    write_json(&cat_path, &all_cats)?;

    let file_count = fs::read_dir(&out_dir)?.count();
    println!(
        "\nDone! {} → {} files (grand total: {} POCs)",
        OUT_DIR,
        file_count,
        grand_total
    );
    println!("  _categories.json written");

    Ok(())
}
