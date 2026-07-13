use clap::Parser;
use nuclei_poc::core::{category, index};

#[derive(Parser, Debug)]
#[command(name = "7_get_pocname", about = "Generate poc_index.json + poc.txt from the poc/ directory")]
struct Args {
    /// Directory to index (default: poc)
    #[arg(long, default_value = "poc")]
    poc_dir: String,

    /// Output directory for index files (default: .)
    #[arg(long, default_value = ".")]
    output_dir: String,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let cmap = category::category_map();

    let (entries, category_counts) =
        index::generate_index(&args.poc_dir, &args.output_dir, &cmap)?;

    // Also write plain poc.txt for backward compatibility
    index::write_poc_txt(&args.output_dir, &entries)?;
    println!("  poc.txt 已生成: {} 条记录", entries.len());

    // Write summary JSON
    index::write_summary_json(&args.output_dir, entries.len(), &category_counts)?;
    println!("  poc_summary.json 已生成");

    // Print top categories
    println!("\n分类统计:");
    let mut sorted: Vec<_> = category_counts.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(a.1));
    for (cat, count) in sorted.iter().take(15) {
        println!("  {}: {}", cat, count);
    }

    Ok(())
}

