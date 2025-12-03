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

/// Clone or update repos in parallel with a concurrency limit.
#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    /// Repository csv file
    #[clap(default_value = "repo.csv")]
    repo_file: String,

    /// clones base directory
    #[clap(short, long, default_value = "clone-templates")]
    clone_dir: String,

    /// maximum concurrent git operations (0 = auto detect)
    #[clap(short, long, default_value_t = 0)]
    jobs: usize,
}
fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let jobs_final = if args.jobs == 0 { let n = num_cpus::get(); if n==0 {1} else {n} } else { args.jobs };
    // Build a tokio runtime with worker_threads=jobs_final
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(jobs_final).enable_all().build()?;
    return rt.block_on(async_main(args, jobs_final));
}

async fn async_main(args: Args, jobs_final: usize) -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let repo_file = args.repo_file;
    let clone_dir = args.clone_dir;
    let jobs = jobs_final;

    fs::create_dir_all(&clone_dir)?;

    // read and dedupe
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
            let _permit = permit; // release when dropped
            if let Some((owner, repo_name)) = parse_owner_repo(&url_clone) {
                let target_dir = format!("{}/{}/{}", clone_dir, owner, repo_name).to_lowercase();
                let target_path = Path::new(&target_dir);
                if target_path.is_dir() {
                    info!(repo = %repo_name, path = %target_dir, "pulling");
                    let mut cmd = Command::new("git");
                    cmd.arg("-C").arg(&target_dir).arg("pull");
                    cmd.stdout(Stdio::null());
                    cmd.stderr(Stdio::piped());
                    match cmd.status().await {
                        Ok(status) if status.success() => info!(repo = %repo_name, "pulled"),
                        Ok(status) => warn!(repo = %repo_name, code = ?status.code(), "git pull failed"),
                        Err(e) => error!(repo = %repo_name, error = %e, "git pull error"),
                    }
                } else {
                    info!(repo = %repo_name, path = %target_dir, "cloning");
                    if let Some(parent) = target_path.parent() { fs::create_dir_all(parent).ok(); }
                    let mut cmd = Command::new("git");
                    cmd.arg("clone").arg(&url_clone).arg(&target_dir);
                    cmd.stdout(Stdio::null());
                    cmd.stderr(Stdio::piped());
                    match cmd.status().await {
                        Ok(status) if status.success() => info!(repo = %repo_name, "cloned"),
                        Ok(status) => warn!(repo = %repo_name, code = ?status.code(), "git clone failed"),
                        Err(e) => error!(repo = %repo_name, error = %e, "git clone error"),
                    }
                }
            } else { warn!(url = %url_clone, "invalid url") }
        });
        handles.push(h);
    }

    for h in handles {
        match h.await {
            Ok(_) => {},
            Err(e) => error!(error = %e, "task join error"),
        }
    }

    info!("All clone tasks completed");
    Ok(())
}

fn parse_owner_repo(url: &str) -> Option<(String, String)> {
    // Expected forms: https://github.com/owner/repo or git@github.com:owner/repo.git
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

