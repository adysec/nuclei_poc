//! POC 校验 — 二阶段流水线异步加速版
//!
//! Phase 0: Auto-fix + SHA256 去重（高 I/O 并发, Semaphore=jobs）
//! Phase 1: nuclei 批量结构校验（每批 N 个文件共享一次进程启动）
//!
//! 相比旧版逐文件启动 nuclei 进程，批量校验可减少 99%+ 的进程创建开销。

use nuclei_poc::core::{hash, yaml};
use std::collections::HashSet;
use std::fs;
use std::os::unix::fs as unix_fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use walkdir::WalkDir;
use clap::Parser;
use tokio::sync::Semaphore;
use futures::stream::StreamExt;
use tracing::{error, warn, info};

// ── 常量 ─────────────────────────────────────────────────────────

const DEFAULT_POC_DIR: &str = "poc";
const DEFAULT_TMP_DIR: &str = "tmp";
const REVIEW_DIR: &str = "poc_needs_review";
const BATCH_TMP: &str = "tmp_nuclei_batches";
const DEFAULT_BATCH_SIZE: usize = 500;

// ── 工具函数 ─────────────────────────────────────────────────────

fn ensure_dir(path: &str) -> anyhow::Result<()> {
    fs::create_dir_all(path)?;
    Ok(())
}

fn move_to_review(src: &Path, review_dir: &str) -> anyhow::Result<()> {
    let rel = src.strip_prefix("tmp").unwrap_or(src);
    let dest = Path::new(review_dir).join(rel);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::rename(src, &dest)?;
    Ok(())
}

fn move_file_dedup(src: &Path, dest: &Path) -> anyhow::Result<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut final_dest = dest.to_path_buf();
    if final_dest.exists() {
        let stem = dest.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
        let ext = dest.extension().and_then(|s| s.to_str()).unwrap_or("");
        let mut counter = 1u32;
        loop {
            let name = if ext.is_empty() {
                format!("{}_{}", stem, counter)
            } else {
                format!("{}_{}.{}", stem, counter, ext)
            };
            final_dest = dest.with_file_name(name);
            if !final_dest.exists() {
                break;
            }
            counter += 1;
        }
    }
    fs::rename(src, &final_dest)?;
    Ok(())
}

/// 从 nuclei 错误输出中提取失败的文件名
fn parse_failed_filenames(err_output: &str) -> HashSet<String> {
    let mut failed = HashSet::new();
    for line in err_output.lines() {
        let trimmed = line.trim();
        if trimmed.contains("FTL") || trimmed.contains("ERR") || trimmed.contains("Error") {
            for quote in ['\'', '"'] {
                let mut start = 0usize;
                while let Some(pos) = trimmed[start..].find(quote) {
                    let abs = start + pos + 1;
                    if let Some(end) = trimmed[abs..].find(quote) {
                        let fname = &trimmed[abs..abs + end];
                        if fname.ends_with(".yaml") || fname.ends_with(".yml") {
                            failed.insert(fname.to_string());
                        }
                    }
                    start = abs + 1;
                }
            }
        }
    }
    failed
}

// ── CLI ───────────────────────────────────────────────────────────

#[derive(Parser, Debug)]
struct Args {
    #[clap(long, default_value = DEFAULT_POC_DIR)]
    poc_dir: String,
    #[clap(long, default_value = DEFAULT_TMP_DIR)]
    tmp_dir: String,
    #[clap(long, default_value = "./nuclei")]
    nuclei_bin: String,
    #[clap(short, long, default_value_t = 0)]
    jobs: usize,
    #[clap(long, default_value_t = 19800u64)]
    timeout_secs: u64,
    #[clap(long, default_value_t = 120u64)]
    per_file_timeout_secs: u64,
    /// 批量校验每批文件数（0=不批量，逐文件校验）
    #[clap(long, default_value_t = DEFAULT_BATCH_SIZE)]
    batch_size: usize,
}

// ── main / runtime ────────────────────────────────────────────────

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let jobs = if args.jobs == 0 { num_cpus::get().max(1) } else { args.jobs };
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(jobs)
        .enable_all()
        .build()?;
    rt.block_on(async_main(args, jobs))
}

async fn async_main(args: Args, jobs: usize) -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let poc_dir = &args.poc_dir;
    let tmp_dir = &args.tmp_dir;
    let nuclei_bin = &args.nuclei_bin;
    let per_file_timeout = Duration::from_secs(args.per_file_timeout_secs);
    let total_timeout = Duration::from_secs(args.timeout_secs);
    let skip_nuclei = !Path::new(nuclei_bin).exists();

    if skip_nuclei {
        warn!("nuclei binary not found at '{}', skipping validation", nuclei_bin);
    }

    info!(
        "POC check start: jobs={}, batch_size={}, timeout={}s, per_file={}s",
        jobs, args.batch_size, args.timeout_secs, args.per_file_timeout_secs
    );

    ensure_dir(poc_dir)?;
    ensure_dir(REVIEW_DIR)?;

    if !Path::new(tmp_dir).exists() {
        println!("tmp/ 目录不存在，退出。");
        return Ok(());
    }

    // 收集 YAML 文件
    let mut yaml_files: Vec<PathBuf> = vec![];
    for entry in WalkDir::new(tmp_dir).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let name = entry.file_name().to_string_lossy();
            if name.ends_with(".yml") || name.ends_with(".yaml") {
                yaml_files.push(entry.path().to_path_buf());
            }
        }
    }

    let total_input = yaml_files.len();
    if total_input == 0 {
        fs::remove_dir_all(tmp_dir).ok();
        println!("tmp/ 目录为空，已删除。");
        return Ok(());
    }

    println!("共发现 {} 个 YAML 文件，开始二阶段校验…\n", total_input);
    let start = Instant::now();

    // ══════════════════════════════════════════════════════════════
    // Phase 0: Auto-fix + SHA256 去重（高 I/O 并发，无 nuclei）
    // ══════════════════════════════════════════════════════════════
    println!("[Phase 0] Auto-fix + SHA256 去重 (并发={})…", jobs);
    let io_sem = Arc::new(Semaphore::new(jobs));

    let total = total_input;
    let processed: Arc<tokio::sync::Mutex<HashSet<String>>> =
        Arc::new(tokio::sync::Mutex::new(HashSet::with_capacity(total)));
    let dups = AtomicU64::new(0);
    let fixed = AtomicU64::new(0);
    let mut survivors = Vec::with_capacity(total);

    let stream = futures::stream::iter(yaml_files.into_iter().enumerate().map(|(i, f)| {
        let io_sem = io_sem.clone();
        let processed = processed.clone();
        let dups = &dups;
        let fixed = &fixed;
        async move {
            let _permit = io_sem.acquire_owned().await.unwrap();
            if i > 0 && i % 50000 == 0 {
                println!("  Phase 0 进度: {}/{}", i, total);
            }

            let f_hash = f.clone();
            let file_hash = tokio::task::spawn_blocking(move || hash::hash_file(&f_hash))
                .await.unwrap()?;

            let mut map = processed.lock().await;
            if map.contains(&file_hash) {
                drop(map);
                let f_rm = f.clone();
                let _ = tokio::task::spawn_blocking(move || {
                    if f_rm.exists() { fs::remove_file(&f_rm)?; }
                    Ok::<_, anyhow::Error>(())
                }).await;
                dups.fetch_add(1, Ordering::Relaxed);
                return Ok::<_, anyhow::Error>(None);
            }
            map.insert(file_hash);
            drop(map);

            let f_fix = f.clone();
            let fix_stats = tokio::task::spawn_blocking(move || -> anyhow::Result<yaml::FixStats> {
                let content = fs::read_to_string(&f_fix)?;
                let (fixed_content, stats) = yaml::auto_fix_poc(&content);
                if stats.total_fixed > 0 {
                    fs::write(&f_fix, fixed_content.as_bytes())?;
                }
                Ok(stats)
            }).await??;
            if fix_stats.total_fixed > 0 {
                fixed.fetch_add(1, Ordering::Relaxed);
            }

            Ok(Some(f))
        }
    })).buffer_unordered(jobs * 2);

    futures::pin_mut!(stream);
    while let Some(result) = stream.next().await {
        match result {
            Ok(Some(f)) => survivors.push(f),
            Ok(None) => {}
            Err(e) => error!("Phase 0 error: {}", e),
        }
        if start.elapsed() >= total_timeout {
            println!("全局超时，中止 Phase 0。");
            break;
        }
    }

    println!(
        "  Phase 0 完成: 去重删除 {} 个，自动修复 {} 个，剩余 {} 个",
        dups.load(Ordering::Relaxed),
        fixed.load(Ordering::Relaxed),
        survivors.len()
    );

    if survivors.is_empty() {
        println!("所有文件均为重复，结束。");
        return Ok(());
    }

    // ══════════════════════════════════════════════════════════════
    // Phase 1: nuclei 批量结构校验
    // ══════════════════════════════════════════════════════════════
    let validated: Vec<PathBuf> = if skip_nuclei || args.batch_size == 0 {
        println!("[Phase 1] nuclei 逐文件结构校验 (并发={})…", jobs);
        phase1_individual(
            &survivors, nuclei_bin, jobs,
            per_file_timeout, total_timeout, &start,
        ).await?
    } else {
        println!("[Phase 1] nuclei 批量结构校验 (每批 {} 个)…", args.batch_size);
        phase1_batch(
            &survivors, nuclei_bin, args.batch_size,
            per_file_timeout, total_timeout, &start,
        ).await?
    };

    println!("  Phase 1 完成: {} 个通过结构校验", validated.len());
    if validated.is_empty() {
        println!("无文件通过校验，结束。");
        return Ok(());
    }

    // 直接将 Phase 1 通过的文件移入 poc/
    println!("文件迁移 (并发={})…", jobs);
    let io_sem2 = Arc::new(Semaphore::new(jobs));
    let passed = Arc::new(AtomicU64::new(0));

    let stream = futures::stream::iter(validated.into_iter().map(|f| {
        let io_sem2 = io_sem2.clone();
        let passed = passed.clone();
        let poc_dir = poc_dir.clone();
        let tmp_dir_str = tmp_dir.to_string();

        async move {
            let _permit = io_sem2.acquire_owned().await.unwrap();
            let rel = f.strip_prefix(&tmp_dir_str).unwrap_or(&f).to_path_buf();
            let dest = Path::new(&poc_dir).join(rel);
            let f_mv = f.clone();
            let _ = tokio::task::spawn_blocking(move || move_file_dedup(&f_mv, &dest)).await;
            passed.fetch_add(1, Ordering::Relaxed);
            Ok::<_, anyhow::Error>(())
        }
    })).buffer_unordered(jobs * 4);

    futures::pin_mut!(stream);
    while let Some(result) = stream.next().await {
        if let Err(e) = result { error!("文件迁移 error: {}", e); }
        if start.elapsed() >= total_timeout { println!("全局超时，中止迁移。"); break; }
    }

    let pass = passed.load(Ordering::Relaxed);
    println!("  文件迁移完成: {} 个", pass);

    // 清理空 tmp/
    if Path::new(tmp_dir).exists() {
        let mut rd = tokio::fs::read_dir(tmp_dir).await?;
        if rd.next_entry().await?.is_none() {
            tokio::fs::remove_dir_all(tmp_dir).await.ok();
            println!("tmp/ 目录已删除。");
        }
    }

    let elapsed = start.elapsed();
    println!(
        "\n=== POC 校验完成 ===\n总输入: {}\n通过: {}\n耗时: {:.1}s",
        total_input, pass, elapsed.as_secs_f64()
    );
    Ok(())
}

// ── Phase 1 实现 ─────────────────────────────────────────────────

async fn phase1_batch(
    files: &[PathBuf],
    nuclei_bin: &str,
    batch_size: usize,
    per_file_timeout: Duration,
    total_timeout: Duration,
    start: &Instant,
) -> anyhow::Result<Vec<PathBuf>> {
    let batch_dir = Path::new(BATCH_TMP);
    if batch_dir.exists() { fs::remove_dir_all(batch_dir)?; }

    let mut passed: Vec<PathBuf> = Vec::with_capacity(files.len());
    let total_batches = files.len().div_ceil(batch_size);

    for (bi, chunk) in files.chunks(batch_size).enumerate() {
        if start.elapsed() >= total_timeout {
            println!("全局超时，中止 Phase 1。");
            break;
        }
        if bi % 50 == 0 || bi == total_batches - 1 {
            println!("  Phase 1 批次: {}/{} ({} 文件/批)", bi + 1, total_batches, chunk.len());
        }

        let sub = batch_dir.join(format!("b{:06}", bi));
        fs::create_dir_all(&sub)?;

        for f in chunk {
            let name = f.file_name().unwrap();
            let link = sub.join(name);
            if link.exists() {
                let stem = f.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
                let ext = f.extension().and_then(|s| s.to_str()).unwrap_or("");
                let mut c = 1u32;
                loop {
                    let alt = sub.join(if ext.is_empty() {
                        format!("{}_{}", stem, c)
                    } else {
                        format!("{}_{}.{}", stem, c, ext)
                    });
                    if !alt.exists() { unix_fs::symlink(f, &alt)?; break; }
                    c += 1;
                }
            } else {
                unix_fs::symlink(f, &link)?;
            }
        }

        let validate_result = tokio::time::timeout(
            per_file_timeout * chunk.len() as u32 / 10 + Duration::from_secs(5),
            tokio::process::Command::new(nuclei_bin)
                .arg("-duc").arg("-validate").arg("-silent")
                .arg("-t").arg(&sub)
                .output(),
        ).await;

        let batch_failed: HashSet<String> = match validate_result {
            Ok(Ok(output)) => {
                let combined = format!(
                    "{}{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
                if output.status.success() && combined.trim().is_empty() {
                    HashSet::new()
                } else {
                    parse_failed_filenames(&combined)
                }
            }
            _ => {
                warn!("批次 {} nuclei 超时/错误，整批移至审阅", bi);
                for f in chunk {
                    let f_q = f.clone();
                    let review = REVIEW_DIR.to_string();
                    let _ = tokio::task::spawn_blocking(move || move_to_review(&f_q, &review)).await;
                }
                let _ = fs::remove_dir_all(&sub);
                continue;
            }
        };

        for f in chunk {
            let fname = f.file_name().unwrap().to_string_lossy().into_owned();
            if batch_failed.contains(&fname) {
                let f_q = f.clone();
                let review = REVIEW_DIR.to_string();
                let _ = tokio::task::spawn_blocking(move || move_to_review(&f_q, &review)).await;
            } else {
                passed.push(f.clone());
            }
        }
        let _ = fs::remove_dir_all(&sub);
    }

    let _ = fs::remove_dir_all(batch_dir);
    println!("  Phase 1 统计: 通过 {}", passed.len());
    Ok(passed)
}

/// Phase 1 逐文件模式（--batch-size 0 时使用）
async fn phase1_individual(
    files: &[PathBuf],
    nuclei_bin: &str,
    jobs: usize,
    per_file_timeout: Duration,
    total_timeout: Duration,
    start: &Instant,
) -> anyhow::Result<Vec<PathBuf>> {
    let sem = Arc::new(Semaphore::new(jobs));
    let passed = Arc::new(tokio::sync::Mutex::new(Vec::with_capacity(files.len())));

    let stream = futures::stream::iter(files.iter().enumerate().map(|(i, f)| {
        let sem = sem.clone();
        let passed = passed.clone();
        let nuclei_bin = nuclei_bin.to_string();
        let f = f.clone();
        async move {
            let _permit = sem.acquire_owned().await.unwrap();
            if i % 10000 == 0 { println!("  Phase 1 进度: {}", i); }

            let df = f.clone();
            let nb = nuclei_bin.clone();
            let validate_ok = match tokio::time::timeout(per_file_timeout, async move {
                let output = tokio::process::Command::new(&nb)
                    .arg("-duc").arg("-validate").arg("-silent")
                    .arg("-t").arg(&df)
                    .output().await?;
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                Ok::<_, anyhow::Error>(
                    output.status.success()
                        && !stdout.contains("FTL")
                        && !stderr.contains("FTL"),
                )
            }).await {
                Ok(Ok(v)) => v,
                _ => false,
            };

            if validate_ok {
                passed.lock().await.push(f.clone());
            } else {
                let f_q = f.clone();
                let review = REVIEW_DIR.to_string();
                let _ = tokio::task::spawn_blocking(move || move_to_review(&f_q, &review)).await;
            }
            Ok::<_, anyhow::Error>(())
        }
    })).buffer_unordered(jobs * 2);

    futures::pin_mut!(stream);
    while let Some(result) = stream.next().await {
        if let Err(e) = result { error!("Phase 1 error: {}", e); }
        if start.elapsed() >= total_timeout { break; }
    }

    Ok(passed.lock().await.clone())
}
