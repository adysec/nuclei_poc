use std::fs::File;
use std::io::{BufWriter, Write};
// no Path import needed
use walkdir::WalkDir;

fn main() -> anyhow::Result<()> {
    let mut files = vec![];
    for entry in WalkDir::new(".").into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if name.ends_with(".yml") || name.ends_with(".yaml") {
                files.push(entry.path().to_path_buf());
            }
        }
    }
    files.sort();
    let f = File::create("poc.txt")?;
    let mut w = BufWriter::new(f);
    for p in files {
        writeln!(w, "{}", p.display())?;
    }
    Ok(())
}

