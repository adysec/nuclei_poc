use clap::Parser;
use std::collections::HashSet;
use std::fs;
use std::io::{self, BufRead};
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::{error, info, warn};

/// 并发克隆或更新仓库，并限制并发数。
#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    /// 仓库列表的CSV文件
    #[clap(default_value = "repo.csv")]
    repo_file: String,

    /// 克隆目标根目录
    #[clap(short, long, default_value = "clone-templates")]
    clone_dir: String,

    /// 最大并发的git操作（0表示自动检测）
    #[clap(short, long, default_value_t = 0)]
    jobs: usize,
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
    let repo_file = args.repo_file;
    let clone_dir = args.clone_dir;
    let jobs = jobs_final;

    fs::create_dir_all(&clone_dir)?;

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
                    match cmd.status().await {
                        Ok(status) if status.success() => info!(repo = %repo_name, "拉取完成"),
                        Ok(status) => warn!(repo = %repo_name, code = ?status.code(), "git pull 失败"),
                        Err(e) => error!(repo = %repo_name, error = %e, "git pull 错误"),
                    }
                } else {
                    info!(repo = %repo_name, path = %target_dir, "开始克隆");
                    if let Some(parent) = target_path.parent() { fs::create_dir_all(parent).ok(); }
                    let mut cmd = Command::new("git");
                    cmd.arg("clone").arg(&url_clone).arg(&target_dir);
                    cmd.stdout(Stdio::null());
                    cmd.stderr(Stdio::piped());
                    match cmd.status().await {
                        Ok(status) if status.success() => info!(repo = %repo_name, "克隆完成"),
                        Ok(status) => warn!(repo = %repo_name, code = ?status.code(), "git clone 失败"),
                        Err(e) => error!(repo = %repo_name, error = %e, "git clone 错误"),
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

