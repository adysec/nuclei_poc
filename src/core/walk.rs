//! YAML-file collection helpers used by multiple pipeline stages.

use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Recursively collect all `.yaml` / `.yml` files under `root`.
pub fn collect_yaml_files(root: &Path) -> Vec<PathBuf> {
    WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file()
                && e.path()
                    .extension()
                    .map_or(false, |ext| ext.eq_ignore_ascii_case("yaml") || ext.eq_ignore_ascii_case("yml"))
        })
        .map(|e| e.path().to_path_buf())
        .collect()
}

/// Recursively count `.yaml` / `.yml` files under `root`.
pub fn count_yaml_files(root: &Path) -> usize {
    WalkDir::new(root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file()
                && e.path()
                    .extension()
                    .map_or(false, |ext| ext.eq_ignore_ascii_case("yaml") || ext.eq_ignore_ascii_case("yml"))
        })
        .count()
}
