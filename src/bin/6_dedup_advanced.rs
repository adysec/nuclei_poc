use anyhow::Result;
use clap::Parser;
use nuclei_poc::core::{features, naming, yaml};
use num_cpus;
use rayon::prelude::*;
use serde_yaml::Value;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use walkdir::WalkDir;

// ============================================================================
// CLI Arguments
// ============================================================================

#[derive(Parser, Debug)]
#[command(
    name = "8_dedup_advanced",
    about = "高级POC去重：多因素评分 + 格式校验 + 文件命名规范化"
)]
struct Args {
    /// 源POC目录（默认使用 nuclei 验证通过的 poc/）
    #[arg(long, default_value = "poc")]
    src_dir: String,

    /// 输出目录
    #[arg(long, default_value = "poc_dedup")]
    dst_dir: String,

    /// 重复判定阈值（0-100），默认70分
    #[arg(long, default_value_t = 70)]
    threshold: i32,

    /// 仅分析不执行
    #[arg(long, default_value_t = false)]
    dry_run: bool,

    /// 显示详细匹配信息
    #[arg(short, long, default_value_t = false)]
    verbose: bool,

    /// 最大并行线程数（0=自动）
    #[arg(long, default_value_t = 0)]
    jobs: usize,
}

// ============================================================================
// Main Dedup Pipeline (uses shared core library for all extraction & scoring)
// ============================================================================

fn main() -> Result<()> {
    let args = Args::parse();
    let src = Path::new(&args.src_dir);
    let dst = Path::new(&args.dst_dir);

    if !src.exists() {
        anyhow::bail!("Source directory does not exist: {}", args.src_dir);
    }

    println!("=== POC高级去重工具 ===");
    println!("源目录: {}", args.src_dir);
    println!("输出目录: {}", args.dst_dir);
    println!("重复阈值: {} 分", args.threshold);
    println!("Dry run: {}", args.dry_run);
    println!();

    // Phase 1: Collect all YAML files
    println!("[Phase 1] 收集所有POC文件...");
    let all_files: Vec<PathBuf> = WalkDir::new(src)
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
    println!("  共发现 {} 个POC文件", all_files.len());

    // Phase 2: Parallel feature extraction (using shared features::extract)
    println!("\n[Phase 2] 提取POC特征（并行处理）...");
    let jobs = if args.jobs == 0 {
        num_cpus::get()
    } else {
        args.jobs
    };
    rayon::ThreadPoolBuilder::new()
        .num_threads(jobs)
        .build_global()
        .ok();

    let poc_features: Vec<features::PocFeatures> = all_files
        .par_iter()
        .enumerate()
        .map(|(i, path)| {
            if i % 50000 == 0 && i > 0 {
                println!("  进度: {}/{}", i, all_files.len());
            }
            features::extract(path)
        })
        .collect();

    let valid_count = poc_features.iter().filter(|f| f.valid).count();
    let invalid_count = poc_features.len() - valid_count;
    println!("  有效POC: {}, 无效POC: {}", valid_count, invalid_count);

    // Phase 3+3b: Parallel validation report + auto-fix statistics
    println!("\n[Phase 3+3b] 并行格式校验与自动修复统计...");
    let phase3_results: Vec<(bool, Vec<String>, Vec<String>, yaml::FixStats)> = poc_features
        .par_iter()
        .filter(|f| f.valid)
        .map(|f| {
            let yaml_str = std::str::from_utf8(&f.raw_content).unwrap_or("");
            let (is_valid, errors, warnings) =
                if let Ok(yaml) = serde_yaml::from_str::<Value>(yaml_str) {
                    let vr = yaml::validate_nuclei_format(
                        &yaml,
                        f.has_http,
                        f.has_requests,
                        f.has_matchers,
                        f.request_count,
                    );
                    (vr.is_valid, vr.errors, vr.warnings)
                } else {
                    (false, vec!["invalid yaml".to_string()], vec![])
                };
            let (_, fs) = yaml::auto_fix_poc(yaml_str);
            (is_valid, errors, warnings, fs)
        })
        .collect();

    let mut valid_format = 0usize;
    let mut invalid_format = 0usize;
    let mut validation_errors: HashMap<String, usize> = HashMap::new();
    let mut validation_warnings: HashMap<String, usize> = HashMap::new();
    let mut total_fix_stats = yaml::FixStats::default();

    for (is_valid, errors, warnings, fs) in &phase3_results {
        if *is_valid {
            valid_format += 1;
        } else {
            invalid_format += 1;
        }
        for e in errors {
            *validation_errors.entry(e.clone()).or_insert(0) += 1;
        }
        for w in warnings {
            *validation_warnings.entry(w.clone()).or_insert(0) += 1;
        }
        total_fix_stats.severity_casing += fs.severity_casing;
        total_fix_stats.severity_empty += fs.severity_empty;
        total_fix_stats.id_spaces += fs.id_spaces;
        total_fix_stats.total_fixed += fs.total_fixed;
    }
    // Free memory early
    drop(phase3_results);

    println!("  格式正确: {}, 格式问题: {}", valid_format, invalid_format);
    if !validation_errors.is_empty() {
        println!("\n  格式错误统计:");
        let mut sorted: Vec<_> = validation_errors.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(a.1));
        for (err, count) in sorted.iter().take(10) {
            println!("    [{}] {}", count, err);
        }
    }
    if !validation_warnings.is_empty() {
        println!("\n  格式警告统计:");
        let mut sorted: Vec<_> = validation_warnings.iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(a.1));
        for (warn, count) in sorted.iter().take(10) {
            println!("    [{}] {}", count, warn);
        }
    }
    println!(
        "  severity大小写修复: {} 处",
        total_fix_stats.severity_casing
    );
    println!("  空severity补全: {} 处", total_fix_stats.severity_empty);
    println!("  ID空格修复: {} 处", total_fix_stats.id_spaces);
    println!("  合计可修复: {} 处", total_fix_stats.total_fixed);

    // Phase 4: Same-ID dedup (keep best quality)
    println!("\n[Phase 4] 基于ID的去重...");
    let mut id_groups: HashMap<String, Vec<usize>> = HashMap::new();
    for (idx, f) in poc_features.iter().enumerate() {
        if !f.valid {
            continue;
        }
        if let Some(ref id) = f.id {
            id_groups.entry(id.clone()).or_default().push(idx);
        }
    }

    let duplicate_ids: Vec<_> = id_groups
        .iter()
        .filter(|(_, v)| v.len() > 1)
        .collect();
    println!(
        "  唯一ID数: {}, 重复ID组: {}",
        id_groups.len(),
        duplicate_ids.len()
    );

    let mut id_removed: HashSet<usize> = HashSet::new();
    let mut id_dup_count = 0usize;

    for (_id, indices) in &id_groups {
        if indices.len() <= 1 {
            continue;
        }
        let mut scored: Vec<(usize, i32)> = indices
            .iter()
            .map(|&idx| (idx, poc_features[idx].quality_score()))
            .collect();
        scored.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| b.0.cmp(&a.0)));
        for (idx, _) in scored.iter().skip(1) {
            if !id_removed.contains(idx) {
                id_removed.insert(*idx);
                id_dup_count += 1;
            }
        }
    }
    println!("  ID去重: 删除 {} 个重复POC (保留质量最高的)", id_dup_count);

    // Phase 5: Multi-factor cross-ID dedup (using shared features::calculate_similarity)
    println!("\n[Phase 5] 多因素评分去重（跨ID检测）...");
    let remaining: Vec<usize> = (0..poc_features.len())
        .filter(|i| poc_features[*i].valid && !id_removed.contains(i))
        .collect();

    // Build indexes via shared helper
    let (cve_index, url_index) = features::build_indexes(&poc_features, &remaining);

    // Find candidate pairs via index collision
    let mut candidate_pairs: HashSet<(usize, usize)> = HashSet::new();
    for indices in cve_index.values() {
        for i in 0..indices.len() {
            for j in i + 1..indices.len() {
                let a = indices[i].min(indices[j]);
                let b = indices[i].max(indices[j]);
                if a != b {
                    candidate_pairs.insert((a, b));
                }
            }
        }
    }
    for indices in url_index.values() {
        for i in 0..indices.len() {
            for j in i + 1..indices.len() {
                let a = indices[i].min(indices[j]);
                let b = indices[i].max(indices[j]);
                if a != b {
                    candidate_pairs.insert((a, b));
                }
            }
        }
    }
    println!("  候选对比对: {}", candidate_pairs.len());

    // Phase 5: Parallel similarity scoring + serial greedy resolve
    let pair_list: Vec<(usize, usize)> = candidate_pairs.into_iter().collect();
    let total_pairs = pair_list.len();
    println!("  并行计算 {} 对相似度...", total_pairs);

    // Step 1: Parallel score all candidate pairs
    let scored_pairs: Vec<(usize, usize, i32, features::MatchDetails)> = pair_list
        .par_iter()
        .filter_map(|&(a, b)| {
            let (score, details) = features::calculate_similarity(&poc_features[a], &poc_features[b]);
            if score >= args.threshold {
                let qa = poc_features[a].quality_score();
                let qb = poc_features[b].quality_score();
                let to_remove = if qa >= qb { b } else { a };
                let kept = if to_remove == b { a } else { b };
                Some((to_remove, kept, score, details))
            } else {
                None
            }
        })
        .collect();

    // Step 2: Sort high-scoring pairs descending (best matches first) for deterministic greedy resolve
    let mut scored_pairs = scored_pairs;
    scored_pairs.par_sort_by(|a, b| b.2.cmp(&a.2));

    // Step 3: Serial greedy resolve — mark lower-quality as removed, skip already-removed
    let mut cross_removed: HashSet<usize> = HashSet::new();
    let mut cross_dup_count = 0usize;
    let mut cross_dup_details: Vec<(PathBuf, PathBuf, i32, String)> = Vec::new();

    for (removed, kept, score, details) in &scored_pairs {
        if cross_removed.contains(removed) || cross_removed.contains(kept) {
            continue;
        }
        cross_removed.insert(*removed);
        cross_dup_count += 1;
        if args.verbose && cross_dup_details.len() < 50 {
            cross_dup_details.push((
                poc_features[*kept].file_path.clone(),
                poc_features[*removed].file_path.clone(),
                *score,
                format!(
                    "id_match={} cve_match={} cnvd_match={} url_full={} url_partial={} matcher={} name_sim={:.2}",
                    details.id_match,
                    details.cve_match,
                    details.cnvd_match,
                    details.url_full_match,
                    details.url_partial_match,
                    details.matcher_similar,
                    details.name_similarity
                ),
            ));
        }
    }
    // Free temp scored_pairs memory
    drop(scored_pairs);
    println!("  跨ID去重: 删除 {} 个重复POC", cross_dup_count);

    // Phase 6: Summary
    println!("\n[Phase 6] 去重总结...");
    let total_removed = id_removed.len() + cross_removed.len();
    let final_count = valid_count - total_removed;
    let all_removed: HashSet<usize> = id_removed.union(&cross_removed).copied().collect();

    println!("  原始有效POC: {}", valid_count);
    println!("  格式无效POC: {}", invalid_count);
    println!("  ID重复删除: {}", id_dup_count);
    println!("  跨ID重复删除: {}", cross_dup_count);
    println!("  总计删除: {}", total_removed);
    println!("  最终保留: {}", final_count);
    println!(
        "  格式自动修复: {} 处 (severity大小写={}, 空severity={}, ID空格={})",
        total_fix_stats.total_fixed,
        total_fix_stats.severity_casing,
        total_fix_stats.severity_empty,
        total_fix_stats.id_spaces
    );

    // Phase 7: Output
    if args.dry_run {
        println!("\n[Dry Run] 不执行实际文件操作。");
        println!("\n[命名分析] 文件命名模式统计:");
        let mut suffix_n = 0usize;
        let mut suffix_numeric = 0usize;
        let mut other_n = 0usize;
        let suffix_re = regex::Regex::new(r"_\d+\.ya?ml$").unwrap();
        let numeric_re = regex::Regex::new(r"-\d{4,}\.ya?ml$").unwrap();
        for f in &poc_features {
            let name = f.file_path.file_name().unwrap().to_string_lossy();
            if suffix_re.is_match(&name) {
                suffix_n += 1;
            } else if numeric_re.is_match(&name) {
                suffix_numeric += 1;
            } else {
                other_n += 1;
            }
        }
        println!("  _N 后缀命名: {} (建议改为标准名)", suffix_n);
        println!("  -NNNN 数字后缀: {} (建议改为标准名)", suffix_numeric);
        println!("  其他命名: {}", other_n);
    } else {
        println!("\n[Phase 7] 并行复制去重后的POC到输出目录...");
        if dst.exists() {
            fs::remove_dir_all(dst)?;
        }
        fs::create_dir_all(dst)?;

        let copied = AtomicU64::new(0);
        let renamed = AtomicU64::new(0);
        let fixed_count = AtomicU64::new(0);

        let to_copy: Vec<&features::PocFeatures> = poc_features
            .iter()
            .enumerate()
            .filter(|(idx, f)| f.valid && !all_removed.contains(idx))
            .map(|(_, f)| f)
            .collect();

        let total_copy = to_copy.len();
        to_copy.par_iter().for_each(|f| {
            let rel = f.file_path.strip_prefix(src).unwrap();
            let category = rel
                .parent()
                .and_then(|p| p.to_str())
                .unwrap_or("other");

            let std_name = naming::standard_filename(
                f.id.as_deref(),
                f.name.as_deref(),
                &f.content_hash,
            );
            let original_name = f.file_path.file_name().unwrap().to_string_lossy();

            let dst_dir = dst.join(category);
            fs::create_dir_all(&dst_dir).ok();
            let dst_path = dst_dir.join(&std_name);

            let final_path = if dst_path.exists() {
                let stem = std_name.trim_end_matches(".yaml");
                dst_dir.join(format!("{}-{}.yaml", stem, &f.content_hash[..8]))
            } else {
                dst_path
            };

            let yaml_str = std::str::from_utf8(&f.raw_content).unwrap_or("");
            let (fixed_content, fs) = yaml::auto_fix_poc(yaml_str);
            if fs.total_fixed > 0 {
                fs::write(&final_path, fixed_content.as_bytes()).ok();
                fixed_count.fetch_add(1, Ordering::Relaxed);
            } else {
                fs::write(&final_path, &f.raw_content).ok();
            }

            if std_name != original_name {
                renamed.fetch_add(1, Ordering::Relaxed);
            }
            let c = copied.fetch_add(1, Ordering::Relaxed) + 1;
            if c % 10000 == 0 {
                println!("  已复制: {}/{} 文件", c, total_copy);
            }
        });

        let copied = copied.load(Ordering::Relaxed) as usize;
        let renamed = renamed.load(Ordering::Relaxed) as usize;
        let fixed_count = fixed_count.load(Ordering::Relaxed) as usize;

        println!(
            "  复制完成: {} 文件 (其中 {} 个已重命名, {} 个已格式修复)",
            copied, renamed, fixed_count
        );

        // Save JSON report
        let json_report_path = dst.join("dedup_report.json");
        let json_report = serde_json::json!({
            "total_files": all_files.len(),
            "valid_pocs": valid_count,
            "invalid_pocs": invalid_count,
            "id_dup_removed": id_dup_count,
            "cross_id_dup_removed": cross_dup_count,
            "total_removed": total_removed,
            "final_count": final_count,
            "threshold": args.threshold,
            "renamed_files": renamed,
            "auto_fixed_files": fixed_count,
            "severity_fixes": total_fix_stats.severity_casing,
            "severity_empty_fixes": total_fix_stats.severity_empty,
            "id_space_fixes": total_fix_stats.id_spaces,
            "total_fixes": total_fix_stats.total_fixed,
            "scoring_rules": {
                "id_match": 100,
                "cve_cnvd_match": 80,
                "url_method_full": 60,
                "url_partial": 40,
                "matcher_similar": "10-30",
                "name_similarity_gt_08": 20,
                "name_similarity_gt_06": 10,
                "quality_score_factors": 18,
                "quality_score_max": 80
            }
        });
        fs::write(
            &json_report_path,
            serde_json::to_string_pretty(&json_report)?,
        )?;
        println!("  报告已保存: {}/dedup_report.json", args.dst_dir);

        // Verbose output
        if args.verbose && !cross_dup_details.is_empty() {
            println!("\n[详细] 跨ID重复示例 (前20条):");
            for (i, (kept, removed, score, reason)) in
                cross_dup_details.iter().enumerate().take(20)
            {
                println!(
                    "  #{:<3} 评分={} | 保留: {} | 删除: {} | {}",
                    i + 1,
                    score,
                    kept.file_name().unwrap().to_string_lossy(),
                    removed.file_name().unwrap().to_string_lossy(),
                    reason
                );
            }
        }
    }

    Ok(())
}

