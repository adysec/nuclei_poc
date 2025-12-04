// 此处不需要 anyhow::Context
use sha2::{Digest, Sha256};
// 使用 dashmap 进行无锁并发映射
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use walkdir::WalkDir;
use clap::Parser;
// use std::process::Command;
use tokio::fs as tokio_fs;
use tokio::sync::Semaphore;
use std::sync::Arc;
use futures::stream::{FuturesUnordered, StreamExt};
use tracing::{error, warn, info};
use dashmap::DashMap;

// 默认目录
const DEFAULT_POC_DIR: &str = "poc_all";
const DEFAULT_TMP_DIR: &str = "tmp";

fn ensure_dir(path: &str) -> anyhow::Result<()> {
    fs::create_dir_all(path)?;
    Ok(())
}

fn safe_remove(path: &Path) -> anyhow::Result<()> {
    if path.exists() {
        fs::remove_file(path)?;
        println!("poc校验失败，已删除文件: {:?}", path);
    }
    Ok(())
}

fn move_file_blocking(src: &Path, dest: &Path) -> anyhow::Result<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut final_dest = dest.to_path_buf();
    if final_dest.exists() {
        // append counter
        let mut counter = 1;
        let file_stem = dest.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
        let ext = dest.extension().and_then(|s| s.to_str()).unwrap_or("");
        loop {
            let name = if ext.is_empty() {
                format!("{}_{}", file_stem, counter)
            } else {
                format!("{}_{}.{}", file_stem, counter, ext)
            };
            final_dest = dest.with_file_name(name);
            if !final_dest.exists() {
                break;
            }
            counter += 1;
        }
    }
    fs::rename(src, final_dest)?;
    println!("POC 校验成功，已移动文件: {:?} -> {:?}", src, dest);
    Ok(())
}

// 注：主流程改为异步调用 nuclei
fn get_file_hash(path: &Path) -> anyhow::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 { break; }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

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
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let jobs_final = if args.jobs == 0 { let n = num_cpus::get(); if n==0 {1} else {n} } else { args.jobs };
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(jobs_final).enable_all().build()?;
    return rt.block_on(async_main(args, jobs_final));
}

async fn async_main(args: Args, jobs_final: usize) -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let poc_dir = args.poc_dir.as_str();
    let tmp_dir = args.tmp_dir.as_str();
    let nuclei_bin = args.nuclei_bin.as_str();
    let jobs = jobs_final;
    let timeout_secs = args.timeout_secs;
    let per_file_timeout_secs = args.per_file_timeout_secs;
    let skip_nuclei_check = !Path::new(nuclei_bin).exists();
    if skip_nuclei_check {
        warn!("nuclei binary not found at '{}', skipping validation (files will be moved without validation)", nuclei_bin);
    }
    info!("Starting POC check: jobs={}, timeout_secs={}, per_file_timeout_secs={}", jobs, timeout_secs, per_file_timeout_secs);

    ensure_dir(poc_dir)?;
    if !Path::new(tmp_dir).exists() {
        println!("tmp/ 目录不存在，退出。");
        return Ok(());
    }

    let mut yaml_files: Vec<PathBuf> = vec![];
    for entry in WalkDir::new(tmp_dir).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let name = entry.file_name().to_string_lossy();
            if name.ends_with(".yml") || name.ends_with(".yaml") {
                yaml_files.push(entry.path().to_path_buf());
            }
        }
    }
    if yaml_files.is_empty() {
        fs::remove_dir_all(tmp_dir).ok();
        println!("tmp/ 目录已删除。");
        return Ok(());
    }

    let start = Instant::now();
    let processed_files_hash: Arc<DashMap<String, PathBuf>> = Arc::new(DashMap::new());
    let sem = Arc::new(Semaphore::new(jobs));
    let mut tasks = FuturesUnordered::new();

    for file_path in yaml_files {
        if start.elapsed() >= Duration::from_secs(timeout_secs) {
            println!("运行时间已超过 {} 秒，强制退出。", timeout_secs);
            break;
        }
        let sem_clone = sem.clone();
        let per_file_timeout_secs_local = per_file_timeout_secs;
        let df = file_path.clone();
        let nuclei_bin = nuclei_bin.to_string();
        let skip_check_local = skip_nuclei_check;
        let poc_dir = poc_dir.to_string();
        let tmp_dir = tmp_dir.to_string();
        let pach = processed_files_hash.clone();
        let task = tokio::spawn(async move {
            // limit parallelism
            let _permit = sem_clone.acquire_owned().await.unwrap();
            // compute hash in blocking thread
            let df_hash = df.clone();
            let file_hash = tokio::task::spawn_blocking(move || get_file_hash(&df_hash)).await??;
            // check duplicates
            if pach.contains_key(&file_hash) {
                let df_rm = df.clone();
                let _ = tokio::task::spawn_blocking(move || safe_remove(&df_rm)).await?;
                return Ok::<(), anyhow::Error>(());
            }
            // validate yaml (blocking)
            let df_check = df.clone();
            let nuclei_bin_check = nuclei_bin.clone();
            // use async tokio::process::Command to run nuclei and capture output with timeout
            let check_fut = async move {
                if skip_check_local { return Ok(true); }
                let mut cmd = tokio::process::Command::new(&nuclei_bin_check);
                cmd.arg("-t").arg(&df_check).arg("-silent");
                // capture output
                match cmd.output().await {
                    Ok(output) => {
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        Ok(!stdout.contains("FTL") && !stderr.contains("FTL"))
                    }
                    Err(e) => Err(anyhow::anyhow!("nuclei command error: {}", e)),
                }
            };
            let per_file_timeout = Duration::from_secs(per_file_timeout_secs_local);
            let valid = match tokio::time::timeout(per_file_timeout, check_fut).await {
                Ok(Ok(v)) => v,
                Ok(Err(e)) => {
                    warn!("nuclei check error: {}", e);
                    false
                }
                Err(_) => {
                    warn!("nuclei check timeout for file: {:?}", &df);
                    false
                }
            };
            if !valid {
                let df_rm = df.clone();
                let _ = tokio::task::spawn_blocking(move || safe_remove(&df_rm)).await?;
                return Ok::<(), anyhow::Error>(());
            }
            // insert hash and move file to poc
            pach.insert(file_hash.clone(), df.clone());
            let rel = df.strip_prefix(&tmp_dir).unwrap_or(&df).to_path_buf();
            let dest_path = Path::new(&poc_dir).join(rel);
            let df_mv = df.clone();
            let _ = tokio::task::spawn_blocking(move || move_file_blocking(&df_mv, &dest_path)).await?;
            Ok::<(), anyhow::Error>(())
        });
        tasks.push(task);
    }
    // collect results
    while let Some(res) = tasks.next().await {
        match res {
            Ok(Ok(())) => (),
            Ok(Err(e)) => error!("task error: {}", e),
            Err(e) => error!("task join error: {}", e),
        }
    }
    let mut rd = tokio_fs::read_dir(tmp_dir).await?;
    if rd.next_entry().await?.is_none() {
        tokio_fs::remove_dir_all(tmp_dir).await.ok();
        println!("tmp/ 目录已删除。");
    }
    println!("POC 检查完成。");
    Ok(())
}

