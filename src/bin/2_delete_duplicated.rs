use anyhow::Context;
use std::env;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use walkdir::WalkDir;

fn calculate_file_hash(path: &std::path::Path) -> anyhow::Result<String> {
    let mut hasher = Sha256::new();
    let mut file = fs::File::open(path)
        .with_context(|| format!("failed to open file for hashing: {:?}", path))?;
    std::io::copy(&mut file, &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()))
}

fn find_and_remove_duplicates(directory: &str) -> anyhow::Result<()> {
    let mut hash_map: HashMap<String, String> = HashMap::new();
    for entry in WalkDir::new(directory).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let filename = entry.file_name().to_str().unwrap_or_default();
            if filename.ends_with(".yml") || filename.ends_with(".yaml") {
                let path = entry.path();
                let file_hash = calculate_file_hash(path)?;
                if let Some(_prev) = hash_map.get(&file_hash) {
                    println!("删除重复文件: {:?}", path);
                    fs::remove_file(path)?;
                } else {
                    hash_map.insert(file_hash, path.to_string_lossy().into_owned());
                }
            }
        }
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();
    let base_directory = args.get(1).map(|s| s.as_str()).unwrap_or("clone-templates");
    find_and_remove_duplicates(base_directory)
}

