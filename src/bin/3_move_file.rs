use dashmap::DashMap;
use dashmap::DashSet;
use nuclei_poc::core::{category, hash, yaml};
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use walkdir::WalkDir;

/// Directory for non-nuclei YAML files quarantined during step 3.
const QUARANTINE_DIR: &str = "poc_excluded";

fn get_all_yaml_files(dir_path: &str) -> HashMap<String, PathBuf> {
    let mut result = HashMap::new();
    for entry in WalkDir::new(dir_path).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let filename = entry.file_name().to_string_lossy().into_owned();
            if filename.ends_with(".yml") || filename.ends_with(".yaml") {
                result.insert(filename, entry.path().to_path_buf());
            }
        }
    }
    result
}

/// Collect SHA-256 hashes of all existing POC files (for dedup) — parallelized via rayon.
fn get_poc_file_hashes(poc_dir_path: &str) -> anyhow::Result<HashSet<String>> {
    // Phase 1: collect file paths (I/O bound, serial is fine)
    let files: Vec<PathBuf> = WalkDir::new(poc_dir_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file() && {
                let n = e.file_name().to_string_lossy();
                n.ends_with(".yml") || n.ends_with(".yaml")
            }
        })
        .map(|e| e.path().to_path_buf())
        .collect();

    // Phase 2: parallel SHA-256 hashing (CPU bound)
    let set: HashSet<String> = files
        .par_iter()
        .filter_map(|path| hash::hash_file(path).ok())
        .collect();

    Ok(set)
}

/// Copy a file to categorized output dirs if its hash is unique (thread-safe).
fn copy_file_if_unique_parallel(
    file_path: &Path,
    base_dir: &str,
    cmap: &HashMap<&str, Vec<&str>>,
    file_hashes: &DashMap<String, DashSet<String>>,
    poc_file_hashes: &HashSet<String>,
) -> anyhow::Result<()> {
    let file_hash = hash::hash_file(file_path)?;
    if poc_file_hashes.contains(&file_hash) {
        return Ok(());
    }
    let categories = category::classify_file(
        &file_path.file_name().unwrap().to_string_lossy(),
        cmap,
    );
    for cat in categories {
        let target_dir = Path::new(base_dir).join(&cat);
        fs::create_dir_all(&target_dir)?;
        let set = file_hashes.entry(cat.clone()).or_insert_with(DashSet::new);
        if set.insert(file_hash.clone()) {
            let dest = target_dir.join(file_path.file_name().unwrap());
            fs::copy(file_path, dest)?;
        }
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let cmap = category::category_map();
    let args: Vec<String> = std::env::args().collect();
    let community_path = args.get(1).map(|s| s.as_str()).unwrap_or("clone-templates");
    let source_of_truth = args
        .get(2)
        .map(|s| s.as_str())
        .unwrap_or("clone-templates/projectdiscovery/nuclei-templates");
    let output_path = args.get(3).map(|s| s.as_str()).unwrap_or("tmp");
    let poc_dir = args.get(4).map(|s| s.as_str()).unwrap_or("poc_all");

    let community = get_all_yaml_files(community_path);
    let nuclei = get_all_yaml_files(source_of_truth);

    let common_templates: HashSet<&String> =
        community.keys().filter(|k| nuclei.contains_key(*k)).collect();
    let file_hashes: DashMap<String, DashSet<String>> = DashMap::new();

    let poc_file_hashes = if Path::new(poc_dir).exists() {
        get_poc_file_hashes(poc_dir)?
    } else {
        HashSet::new()
    };

    let quarantined = AtomicU64::new(0);
    let total_files = AtomicU64::new(0);

    community.par_iter().for_each(|(template, community_file)| {
        total_files.fetch_add(1, Ordering::Relaxed);
        if common_templates.contains(template) {
            let community_meta = fs::metadata(community_file);
            let nuclei_meta = fs::metadata(&nuclei[template]);
            if let (Ok(cm), Ok(nm)) = (community_meta, nuclei_meta) {
                if cm.len() == nm.len() {
                    let _ = fs::remove_file(community_file);
                    return;
                }
            }
        }

        // Pre-filter: quarantine files that are clearly NOT nuclei templates
        let content = match fs::read_to_string(community_file) {
            Ok(c) => c,
            Err(_) => {
                quarantined.fetch_add(1, Ordering::Relaxed);
                return;
            }
        };
        let (is_tpl, _reason) = yaml::is_nuclei_template(&content);
        if !is_tpl {
            let fname = community_file
                .file_name()
                .unwrap_or_default()
                .to_string_lossy();
            let fname_str: &str = fname.as_ref();
            let categories = category::classify_file(fname_str, &cmap);
            let cat = categories.first().map(|s| s.as_str()).unwrap_or("other");
            let mut dest = Path::new(QUARANTINE_DIR).join(cat).join(fname_str);

            if dest.exists() {
                let stem = dest
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("file")
                    .to_string();
                let ext = dest
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();
                let mut c = 1u32;
                loop {
                    let name = if ext.is_empty() {
                        format!("{}_{}", stem, c)
                    } else {
                        format!("{}_{}.{}", stem, c, ext)
                    };
                    dest = Path::new(QUARANTINE_DIR).join(cat).join(&name);
                    if !dest.exists() {
                        break;
                    }
                    c += 1;
                }
            }
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).ok();
            }
            let _ = fs::rename(community_file, &dest);
            quarantined.fetch_add(1, Ordering::Relaxed);
            return;
        }

        if let Err(e) = copy_file_if_unique_parallel(
            community_file,
            output_path,
            &cmap,
            &file_hashes,
            &poc_file_hashes,
        ) {
            eprintln!("复制失败 {:?}: {e}", community_file);
        }
    });
    println!(
        "文件移动和分类完成。总计: {}, 隔离非nuclei文件: {}",
        total_files.load(Ordering::Relaxed),
        quarantined.load(Ordering::Relaxed)
    );
    Ok(())
}

