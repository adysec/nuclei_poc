//! nuclei_poc shared library — common utilities used across pipeline stages.
//!
//! This crate provides:
//! - `hash`     — SHA256 file/content hashing (unified across all stages)
//! - `walk`     — YAML file collection helpers
//! - `yaml`     — CVE/CNVD extraction, severity normalization, format validation & auto-fix
//! - `category` — Multi-source file classification (filename + tags + path)
//! - `naming`   — Standardised output filename generation
//! - `features` — POC feature extraction & multi-factor similarity scoring
//! - `index`    — Structured JSON index generation (poc_index.json)

pub mod core;
