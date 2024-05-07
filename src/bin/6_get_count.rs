use nuclei_poc::core::hash;
use std::collections::HashSet;
use std::env;
use std::fs;

fn count_files_in_categories(base_dir: &str) -> anyhow::Result<(std::collections::HashMap<String, usize>, HashSet<String>)> {
    let mut category_counts = std::collections::HashMap::new();
    let mut unique_hashes = HashSet::new();
    for entry in fs::read_dir(base_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            let category = entry.file_name().to_string_lossy().into_owned();
            let mut file_count = 0usize;
            for f in fs::read_dir(entry.path())? {
                let f = f?;
                if f.file_type()?.is_file() {
                    file_count += 1;
                    let h = hash::hash_file(&f.path())?;
                    unique_hashes.insert(h);
                }
            }
            category_counts.insert(category, file_count);
        }
    }
    Ok((category_counts, unique_hashes))
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();
    let output_path = args.get(1).map(|s| s.as_str()).unwrap_or("poc");
    let (category_counts, unique_hashes) = count_files_in_categories(output_path)?;
    println!("各类文件数量:");
    let mut total_files = 0usize;
    for (category, count) in &category_counts {
        println!("{}: {}", category, count);
        total_files += count;
    }
    println!("总文件数量: {}", total_files);
    println!("去重后文件数量: {}", unique_hashes.len());
    Ok(())
}

