# Nuclei POCs


<a href="https://github.com/adysec/nuclei_poc/stargazers"><img alt="GitHub Repo stars" src="https://img.shields.io/github/stars/adysec/nuclei_poc?color=yellow&logo=riseup&logoColor=yellow&style=flat-square"></a>
<a href="https://github.com/adysec/nuclei_poc/network/members"><img alt="GitHub forks" src="https://img.shields.io/github/forks/adysec/nuclei_poc?color=orange&style=flat-square"></a>
<a href="https://github.com/adysec/nuclei_poc/issues"><img alt="GitHub issues" src="https://img.shields.io/github/issues/adysec/nuclei_poc?color=red&style=flat-square"></a>

Nuclei POC — updated daily

[中文](https://github.com/adysec/nuclei_poc/blob/main/README.md) | [English](https://github.com/adysec/nuclei_poc/blob/main/readme_en.md)

This project has been migrated and rewritten from Python to Rust. The rewrite improves performance significantly when operating on large repositories and validating PoCs — on GitHub Actions the execution time decreased from ~6 hours (old Python version) to ~6 minutes (Rust release binaries), depending on concurrency and environment.

After the migration the project also improved PoC validation and deduplication logic. To avoid losing PoCs while rolling out new deduplication rules, and to make it easy to compare outputs, the project uses a staged (gray-release) approach and keeps two sets of outputs concurrently.

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
- `poc_all/` — Full PoC outputs (archive of raw / historical results)
- `poc_high_quality/` — Gray-release output after new deduplication / high-quality filtering
- `poc/` — Category-organized PoCs, ready for tools like Nuclei
- `poc.txt` — Text list of archived PoCs for quick reference
- `src/bin/` — Rust source files for pipeline stages (each file corresponds to an executable)
- `target/` — Cargo build output (includes release binaries)

### Pipeline stages

Each `src/bin/<n>_<name>.rs` implements one step of the pipeline:

1. 1_clone_repos — Clone or update repositories listed in `repo.csv` in bulk.
2. 2_delete_duplicated — First-pass deduplication to remove obvious duplicate PoC files.
3. 3_move_file — Archive and organize cleaned PoCs into category directories.
4. 4_download_nuclei — Download / prepare the Nuclei engine (if needed) for validation.
5. 5_check_poc — Validate PoC effectiveness and move valid results to the output directories (`poc/` or other targets).
6. 6_get_count — Count archived PoCs and output statistics.
7. 7_get_pocname — Generate / update the `poc.txt` index for quick lookup.
8. 8_dedup_high_quality — New deduplication and high-quality filter (gray-release) that outputs `poc_high_quality/`.

### Output strategy & safe rollback

To avoid accidental data loss while iterating on deduplication/filtering rules, the project deliberately keeps two outputs during rollout:

- `poc_all/`: complete archive of produced artifacts for auditing and rollback.
- `poc_high_quality/`: the stricter, new-filtered set intended for downstream validation and publishing.

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
