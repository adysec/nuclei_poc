# Nuclei POCs


<a href="https://github.com/adysec/nuclei_poc/stargazers"><img alt="GitHub Repo stars" src="https://img.shields.io/github/stars/adysec/nuclei_poc?color=yellow&logo=riseup&logoColor=yellow&style=flat-square"></a>
<a href="https://github.com/adysec/nuclei_poc/network/members"><img alt="GitHub forks" src="https://img.shields.io/github/forks/adysec/nuclei_poc?color=orange&style=flat-square"></a>
<a href="https://github.com/adysec/nuclei_poc/issues"><img alt="GitHub issues" src="https://img.shields.io/github/issues/adysec/nuclei_poc?color=red&style=flat-square"></a>

Nuclei POC — updated daily

[中文](https://github.com/adysec/nuclei_poc/blob/main/README.md) | [English](https://github.com/adysec/nuclei_poc/blob/main/readme_en.md)

This project has been migrated and rewritten from Python to Rust. The rewrite improves performance significantly when operating on large repositories and validating PoCs — on GitHub Actions the execution time decreased from ~6 hours (old Python version) to ~6 minutes (Rust release binaries), depending on concurrency and environment.

After the migration the project also improved PoC validation and deduplication logic. To avoid losing PoCs while rolling out new deduplication rules, and to make it easy to compare outputs, the project uses a staged (gray-release) approach and keeps three sets of outputs concurrently.

## How to use

Clone the repository and enter the directory:

```bash
git clone https://github.com/adysec/nuclei_poc
cd nuclei_poc
```

Use Nuclei to run PoC scans against a target URL:

```bash
./nuclei -t poc/ -u http://example.com
# Scan only a subset of PoCs
./nuclei -t poc/web/ -u http://example.com
./nuclei -t poc/wordpress/ -u http://example.com
```

### Configuration

Configure the list of monitored GitHub repositories in the `repo.csv` file. After migrating to Rust, the core pipeline is delivered as separate Rust binaries — the source files are in `src/bin/`.

### Build & Run (Rust)

For best performance, build release binaries in the target environment:

```bash
# Build release binaries
cargo build --release

# Run a single pipeline stage (example)
./target/release/1_clone_repos

# Or run in development mode using cargo (useful for debugging)
cargo run --bin 1_clone_repos
```

In production it's recommended to build with `--release` and run pipeline stages in CI or containers with appropriate concurrency and resource limits to achieve the short processing time (~6 minutes in our CI, depends on config and machine).

### GitHub Action

Set up a GitHub Action workflow to run the pipeline daily. The workflow requires `Workflow permissions` to be set to `Read and write`.

## Project layout (overview)

### Top-level files and directories

- `Cargo.toml` — Rust project configuration and dependencies
- `repo.csv` — List of GitHub repositories to monitor/collect (input)
- `poc_non_nuclei/` — Non-nuclei YAML files intercepted at step 3 (docker-compose.yml, etc.), kept for audit
- `poc_needs_review/` — Files that failed nuclei validation at step 5, kept for manual review
- `poc_all/` — Full PoC outputs (archive of raw / historical results)
- `poc_baseline/` — Simple scoring dedup baseline output (step 8, for comparison)
- `poc_high_quality/` — Gray-release output after advanced multi-factor deduplication + format repair (step 9)
- `poc/` — Category-organized PoCs validated by Nuclei, ready for direct scanning
- `poc.txt` — Text list of archived PoCs for quick reference
- `src/core/` — Shared library (hashing, YAML parsing/validation/repair, categorization, naming conventions, feature extraction, JSON indexing)
- `src/bin/` — Rust source files for pipeline stages (each file corresponds to an executable)
- `target/` — Cargo build output (includes release binaries)

### Pipeline stages

Each `src/bin/<n>_<name>.rs` implements one step of the pipeline:

1. 1_clone_repos — Clone or update repositories listed in `repo.csv` in bulk.
2. 2_delete_duplicated — First-pass deduplication to remove obvious duplicate PoC files.
3. 3_move_file — Pre-filter non-nuclei files (→ `poc_non_nuclei/`), then archive and organize cleaned PoCs into `tmp/` and `poc_all/`.
4. 4_download_nuclei — Download / prepare the Nuclei engine (if needed) for validation.
5. 5_check_poc — Apply `auto_fix_poc()` to repair common issues, then run nuclei validation; pass→`poc/`, fail→`poc_needs_review/` (never deletes).
6. 6_get_count — Count archived PoCs and output statistics.
7. 7_get_pocname — Generate structured index: `poc_index.json` (category/quality-score/CVE metadata), `poc_summary.json` (stats summary), and `poc.txt` (plaintext list).
8. 8_dedup_high_quality — Simple deduplication & quality filter (scoring + content hash), outputs to `poc_baseline/` for baseline comparison.
9. 9_dedup_advanced — Advanced deduplication & format repair (multi-factor scoring + CVE/URL dual-index + auto-fix severity/ID casing + naming standardization + JSON report), outputs to `poc_high_quality/`.

### Output strategy & safe rollback

To avoid accidental data loss while iterating on deduplication/filtering rules, the project deliberately keeps three outputs during rollout:

- `poc_all/`: complete archive of produced artifacts for auditing and rollback.
- `poc/`: step 5 Nuclei-validated and category-organized PoCs, ready for direct scanning.
- `poc_baseline/`: step 8 simple scoring + content hash dedup output, for baseline comparison.
- `poc_high_quality/`: step 9 advanced multi-factor deduplication + format-fixed final high-quality output, intended for strict downstream validation and publishing.

Step 9 scoring rules (0-80, 18 factors):
- Basic structure (0-7): id, name, severity
- Severity level (0-8): critical=8, high=6, medium=4, low=2, info=1
- Protocol support (0-10): http+matchers=6, requests+matchers=5, tcp/dns=3ea
- Metadata richness (0-16): author/description/tags/reference/classification/remediation ×2
- Detection capability (0-15): matchers=5, extractors=4, URL count (≤6)
- Vulnerability association (0-10): CVE=6, CNVD=4
- Format normalization (0-6): http (not requests)=3, no severity casing issues=1, no deprecated network=2
- File size appropriateness (0-5): 500B-10KB=5, 200-20KB=3, <200=1
- Multi-protocol bonus (0-3): 2+ protocols=3

You can build and run stages individually while debugging or testing:

```bash
# Build release
cargo build --release

# Run a single stage (e.g. step 1)
./target/release/1_clone_repos

# Or use cargo during development for debugging
cargo run --bin 1_clone_repos -- <args...>
```

## Acknowledgements

Thanks to the open-source community and individuals who supported the project.

### Projects

Special thanks to [ProjectDiscovery](https://github.com/projectdiscovery/nuclei) for developing the Nuclei tool and for their community support.

### People

Thanks to [TajangSec](https://github.com/TajangSec) for code optimizations and suggestions.
Thanks to [重剑无锋](https://github.com/TideSec) for the suggestions on improving the deduplication rules.
