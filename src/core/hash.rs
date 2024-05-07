//! SHA256 hashing utilities — the single canonical hash used across the entire pipeline.
//!
//! We use SHA-256 for both content deduplication and per-stage file comparison.
//! This replaces the mixed MD5+SHA256 situation in the original codebase.

use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::Path;

/// Compute SHA-256 hex digest of a file's content.
///
/// Returns `anyhow::Error` when the file cannot be opened or read.
pub fn hash_file(path: &Path) -> anyhow::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192]; // 8 KiB buffer — good balance of syscalls vs. stack
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 { break; }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// Compute SHA-256 hex digest of an in-memory byte slice.
pub fn hash_bytes(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}
