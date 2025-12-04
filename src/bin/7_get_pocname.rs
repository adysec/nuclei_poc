use std::fs::File;
use std::io::{BufWriter, Write};
use walkdir::WalkDir;

fn main() -> anyhow::Result<()> {
    let mut files: Vec<_> = WalkDir::new(".")
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_lowercase();
            name.ends_with(".yml") || name.ends_with(".yaml")
        })
        .map(|e| e.path().to_path_buf())
        .collect();

    files.sort_unstable();

    let f = File::create("poc.txt")?;
    let mut w = BufWriter::new(f);
    for p in files.into_iter() {
        writeln!(w, "{}", p.display())?;
    }
    Ok(())
}

