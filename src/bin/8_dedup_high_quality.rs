use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;
use serde_yaml::Value;
use clap::Parser;
use rayon::prelude::*;
// removed report serialization

#[derive(Parser, Debug)]
#[command(name = "8_dedup_high_quality", about = "筛选高质量POC并重建目录结构")]
struct Args {
    /// 选择POC的最低得分
    #[arg(short, long, default_value_t = 5)]
    min_score: i32,

    /// 仅演练：不拷贝文件，只输出统计
    #[arg(long, default_value_t = false)]
    dry_run: bool,

    /// 必须存在的顶级字段（逗号分隔）
    #[arg(long, value_delimiter = ',', default_value = "id,info,requests")]
    require_fields: Vec<String>,

    /// 允许的严重性（逗号分隔），为空则不限制
    #[arg(long, value_delimiter = ',', default_value = "critical,high,medium")]
    allow_severities: Vec<String>,

    /// 最少请求数量
    #[arg(long, default_value_t = 1)]
    min_requests: usize,

    /// 是否要求至少一个请求包含匹配器
    #[arg(long, default_value_t = false)]
    require_matchers: bool,

    /// 源路径白名单子串匹配（如仓库路径或标签）
    #[arg(long, value_delimiter = ',', default_value = "")]
    source_whitelist: Vec<String>,
}

fn score_yaml(yaml: &Value) -> i32 {
    let mut score = 0;

    // 基本元数据
    if yaml.get("id").is_some() { score += 2; }
    if yaml.get("info").is_some() { score += 2; }

    if let Some(info) = yaml.get("info") {
        if info.get("name").is_some() { score += 1; }
        if info.get("severity").is_some() { score += 2; }
        if let Some(sev) = info.get("severity").and_then(|v| v.as_str()) {
            match sev {
                "high" | "critical" => score += 3,
                "medium" => score += 2,
                "low" => score += 1,
                _ => {}
            }
        }
        if info.get("description").is_some() { score += 1; }
        if info.get("tags").is_some() { score += 1; }
        if info.get("references").is_some() { score += 1; }
    }

    // 请求丰富度
    if let Some(requests) = yaml.get("requests") {
        if let Some(arr) = requests.as_sequence() {
            let count = arr.len();
            if count >= 1 { score += 2; }
            if count >= 2 { score += 1; }
            if count >= 4 { score += 1; }

            // 是否包含匹配器/提取器
            for req in arr {
                if req.get("matchers").is_some() { score += 2; }
                if req.get("extractors").is_some() { score += 1; }
                if req.get("path").is_some() { score += 1; }
                if req.get("method").is_some() { score += 1; }
            }
        }
    }

    // 其他常见信号
    if yaml.get("variables").is_some() { score += 1; }
    if yaml.get("http").is_some() { score += 1; }
    if yaml.get("tcp").is_some() { score += 1; }
    if yaml.get("headless").is_some() { score += 1; }
    if yaml.get("flows").is_some() { score += 1; }

    score
}

fn main() -> Result<()> {
    let args = Args::parse();
    let src_root = Path::new("poc_all");
    let dst_root = Path::new("poc_high_quality");

    // 每次运行清理目标目录
    if dst_root.exists() {
        fs::remove_dir_all(dst_root).context("Failed to remove poc_high_quality directory")?;
    }
    fs::create_dir_all(dst_root).context("Failed to create poc_high_quality directory")?;

    // 收集所有源文件
    let files: Vec<PathBuf> = WalkDir::new(src_root)
        .into_iter()
        .filter_map(|e| e.ok())
        .map(|e| e.path().to_path_buf())
        .filter(|p| p.is_file())
        .collect();

    // 通过内容哈希去重，并保持目录结构拷贝
    let mut seen: HashSet<String> = HashSet::new();
    let mut considered = 0usize;
    let mut selected = 0usize;

    // 并行评估文件
    let evaluated: Vec<(PathBuf, i32, Vec<String>, Vec<u8>, String)> = files
        .par_iter()
        .map(|file| {
            let mut reasons: Vec<String> = Vec::new();
            let mut buf = Vec::new();
            let mut score = 0;
            let mut hash = String::new();
            match fs::File::open(file) {
                Ok(mut f) => {
                    if let Err(_) = f.read_to_end(&mut buf) {
                        reasons.push("read failed".to_string());
                    } else {
                        hash = format!("{:x}", Sha256::digest(&buf));
                        match serde_yaml::from_slice::<Value>(&buf) {
                            Ok(v) => {
                                score = score_yaml(&v);
                                for req in &args.require_fields {
                                    if !v.get(req).is_some() {
                                        reasons.push(format!("missing field: {}", req));
                                    }
                                }
                                if let Some(info) = v.get("info") {
                                    if let Some(sev) = info.get("severity").and_then(|s| s.as_str()) {
                                        if !args.allow_severities.is_empty()
                                            && !args.allow_severities.iter().any(|a| a.eq_ignore_ascii_case(sev))
                                        {
                                            reasons.push(format!("severity not allowed: {}", sev));
                                        }
                                    } else {
                                        reasons.push("missing severity".to_string());
                                    }
                                } else {
                                    reasons.push("missing info".to_string());
                                }
                                let mut req_count = 0usize;
                                let mut has_matchers = false;
                                if let Some(requests) = v.get("requests").and_then(|r| r.as_sequence()) {
                                    req_count = requests.len();
                                    for req in requests {
                                        if req.get("matchers").is_some() { has_matchers = true; }
                                    }
                                } else {
                                    reasons.push("missing requests".to_string());
                                }
                                if req_count < args.min_requests {
                                    reasons.push(format!("requests too few: {} < {}", req_count, args.min_requests));
                                }
                                if args.require_matchers && !has_matchers {
                                    reasons.push("no matchers".to_string());
                                }
                                if !args.source_whitelist.is_empty() {
                                    let p = file.display().to_string();
                                    if !args.source_whitelist.iter().any(|s| !s.is_empty() && p.contains(s)) {
                                        reasons.push("source not whitelisted".to_string());
                                    }
                                }
                            }
                            Err(_) => {
                                reasons.push("invalid yaml".to_string());
                            }
                        }
                    }
                }
                Err(_) => reasons.push("open failed".to_string()),
            }
            (file.clone(), score, reasons, buf, hash)
        })
        .collect();

    // 复制到 poc_high_quality
    for (file, score, reasons, buf, hash) in evaluated {
        considered += 1;
        let missing_required = reasons.iter().any(|r| r.starts_with("missing field"));
        if score >= args.min_score && !missing_required {
            if !seen.contains(&hash) {
                seen.insert(hash);
                selected += 1;
                if !args.dry_run {
                    let rel = file.strip_prefix(src_root).unwrap();
                    let dst_path = dst_root.join(rel);
                    if let Some(parent) = dst_path.parent() {
                        fs::create_dir_all(parent).with_context(|| format!("Create dir failed: {}", parent.display()))?;
                    }
                    fs::write(&dst_path, &buf)
                        .with_context(|| format!("Write file failed: {}", dst_path.display()))?;
                }
            }
        }
    }

    println!(
        "已评估 {} 个文件，选中 {} 个，唯一拷贝 {} 个（min_score={}）",
        considered,
        selected,
        seen.len(),
        args.min_score
    );
    Ok(())
}

