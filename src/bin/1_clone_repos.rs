use clap::Parser;
use std::collections::HashSet;
use std::fs;
use std::io::{self, BufRead};
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::time::timeout;
use tracing::{error, info, warn};

/// 并发克隆或更新仓库，并限制并发数。
/// 克隆完成后会将已有的 poc/ 目录移入 clone-templates/，
/// 保证循环运行时所有文件都参与后续去重流程。
#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    /// 仓库列表的CSV文件
    #[clap(default_value = "repo.csv")]
    repo_file: String,

    /// 克隆目标根目录
    #[clap(short, long, default_value = "clone-templates")]
    clone_dir: String,

    /// 已有的 poc 目录（支持多个，会移入 clone-templates 参与重处理）。
    /// 默认：poc poc_gold_11 poc_gold_12 poc_gold_13 poc_gold_14 poc_gold_15 poc_dedup poc_excluded
    #[clap(long, default_values = &["poc", "poc_gold_11", "poc_gold_12", "poc_gold_13", "poc_gold_14", "poc_gold_15", "poc_dedup", "poc_excluded"])]
    poc_dirs: Vec<String>,

    /// 最大并发的git操作（0表示自动检测）
    #[clap(short, long, default_value_t = 0)]
    jobs: usize,

    /// 跳过 git clone/pull，仅将 poc/ 复制到 clone-templates/
    #[clap(long)]
    skip_clone: bool,
}
fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let jobs_final = if args.jobs == 0 { let n = num_cpus::get(); if n==0 {1} else {n} } else { args.jobs };
    // 构建Tokio运行时，工作线程数=jobs_final
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(jobs_final).enable_all().build()?;
    return rt.block_on(async_main(args, jobs_final));
}

async fn async_main(args: Args, jobs_final: usize) -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    // 预检查：git 是否可用
    match which::which("git") {
        Ok(path) => info!(git = %path.display(), "git 可用"),
        Err(e) => {
            error!(error = %e, "git 未找到，请安装 git 后重试");
            return Err(anyhow::anyhow!("git not found: {}", e));
        }
    }

    let repo_file = args.repo_file;
    let clone_dir = args.clone_dir;
    let jobs = jobs_final;

    fs::create_dir_all(&clone_dir)?;

    if args.skip_clone {
        info!("跳过 git clone/pull (--skip-clone)");
    } else {
        // 读取并去重
    let file = fs::File::open(&repo_file).map_err(|e| anyhow::anyhow!("open {}: {}", repo_file, e))?;
    let reader = io::BufReader::new(file);
    let mut urls: HashSet<String> = HashSet::new();
    for line in reader.lines().filter_map(Result::ok) {
        let s = line.trim().to_string();
        if !s.is_empty() { urls.insert(s); }
    }

    info!(url_count = urls.len(), jobs, "Starting cloning with concurrency");

    let sem = Arc::new(Semaphore::new(jobs));
    let mut handles = vec![];
    for url in urls {
        let sem_clone = sem.clone();
        let permit = sem_clone.acquire_owned().await.unwrap();
        let clone_dir = clone_dir.clone();
        let url_clone = url.clone();
        let h = tokio::spawn(async move {
            let _permit = permit; // 作用域结束自动释放
            if let Some((owner, repo_name)) = parse_owner_repo(&url_clone) {
                let target_dir = format!("{}/{}/{}", clone_dir, owner, repo_name).to_lowercase();
                let target_path = Path::new(&target_dir);
                if target_path.is_dir() {
                    info!(repo = %repo_name, path = %target_dir, "拉取更新");
                    let mut cmd = Command::new("git");
                    cmd.arg("-C").arg(&target_dir).arg("pull");
                    cmd.stdout(Stdio::null());
                    cmd.stderr(Stdio::piped());
                    cmd.kill_on_drop(true);
                    match timeout(Duration::from_secs(120), cmd.output()).await {
                        Ok(Ok(output)) if output.status.success() => {
                            info!(repo = %repo_name, "拉取完成");
                        }
                        Ok(Ok(output)) => {
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            let stderr_short: String = stderr.lines().take(5).collect::<Vec<_>>().join(" | ");
                            warn!(repo = %repo_name, code = ?output.status.code(), stderr = %stderr_short, "git pull 失败");
                        }
                        Ok(Err(e)) => {
                            error!(repo = %repo_name, error = %e, "git pull 错误");
                        }
                        Err(_elapsed) => {
                            warn!(repo = %repo_name, "git pull 超时 (120s)");
                        }
                    }
                } else {
                    info!(repo = %repo_name, path = %target_dir, "开始克隆");
                    if let Some(parent) = target_path.parent() { fs::create_dir_all(parent).ok(); }
                    let mut cmd = Command::new("git");
                    cmd.arg("clone").arg(&url_clone).arg(&target_dir);
                    cmd.stdout(Stdio::null());
                    cmd.stderr(Stdio::piped());
                    cmd.kill_on_drop(true);
                    match timeout(Duration::from_secs(120), cmd.output()).await {
                        Ok(Ok(output)) if output.status.success() => {
                            info!(repo = %repo_name, "克隆完成");
                        }
                        Ok(Ok(output)) => {
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            let stderr_short: String = stderr.lines().take(5).collect::<Vec<_>>().join(" | ");
                            warn!(repo = %repo_name, code = ?output.status.code(), stderr = %stderr_short, "git clone 失败");
                        }
                        Ok(Err(e)) => {
                            error!(repo = %repo_name, error = %e, "git clone 错误");
                        }
                        Err(_elapsed) => {
                            warn!(repo = %repo_name, "git clone 超时 (120s)");
                        }
                    }
                }
            } else { warn!(url = %url_clone, "URL 无效") }
        });
        handles.push(h);
    }

    for h in handles {
        match h.await {
            Ok(_) => {},
            Err(e) => error!(error = %e, "任务合并错误"),
        }
    }

    info!("所有克隆任务已完成");

    } // end of clone block

    // ── 将所有已有的 poc* 目录移入 clone-templates/ ──
    for poc_dir in &args.poc_dirs {
        let poc_src = Path::new(poc_dir);
        if !poc_src.is_dir() {
            info!("{} 目录不存在，跳过", poc_dir);
            continue;
        }
        let dir_name = poc_src.file_name().unwrap_or_default();
        let poc_dest = Path::new(&clone_dir).join(dir_name);
        if poc_dest.exists() {
            if let Err(e) = fs::remove_dir_all(&poc_dest) {
                warn!("无法清理旧目录 {:?}: {}", poc_dest, e);
            }
        }
        info!("移动已有 POC: {:?} -> {:?}", poc_src, poc_dest);
        match fs::rename(poc_src, &poc_dest) {
            Ok(()) => info!("已移动到 {:?}", poc_dest),
            Err(e) => warn!("移动 {} 失败: {} (原目录保留)", poc_dir, e),
        }
    }

    Ok(())
}

fn parse_owner_repo(url: &str) -> Option<(String, String)> {
    // 期望形式示例: https://github.com/owner/repo 或 git@github.com:owner/repo.git
    if let Some(idx) = url.rfind('/') {
        let repo_name = url[idx+1..].trim_end_matches('.').trim_end_matches(".git");
        // get owner before last '/'
        let owner_part = &url[..idx];
        if let Some(idx2) = owner_part.rfind('/') {
            let owner = owner_part[idx2+1..].to_string();
            return Some((owner, repo_name.to_string()));
        }
    }
    None
}

