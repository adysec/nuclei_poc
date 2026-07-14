# Nuclei POCs

<a href="https://github.com/adysec/nuclei_poc/stargazers"><img alt="GitHub Repo stars" src="https://img.shields.io/github/stars/adysec/nuclei_poc?color=yellow&logo=riseup&logoColor=yellow&style=flat-square"></a>
<a href="https://github.com/adysec/nuclei_poc/network/members"><img alt="GitHub forks" src="https://img.shields.io/github/forks/adysec/nuclei_poc?color=orange&style=flat-square"></a>
<a href="https://github.com/adysec/nuclei_poc/issues"><img alt="GitHub issues" src="https://img.shields.io/github/issues/adysec/nuclei_poc?color=red&style=flat-square"></a>

A curated collection of Nuclei PoCs — automatically collected, validated, deduplicated, and published daily.

[中文](https://github.com/adysec/nuclei_poc/blob/main/README.md) | [English](https://github.com/adysec/nuclei_poc/blob/main/README_EN.md)

Migrated from Python to Rust. Full pipeline execution on GitHub Actions dropped from ~6 hours to ~6 minutes.

---

## Outputs

Four directories cover the full spectrum from raw validation to curated gold:

| Directory | Count | Source | Use Case |
|-----------|-------|--------|----------|
| `poc/` | ~600k | Step 4 — nuclei-validated | Daily scanning, categorized, ready for `-t poc/` |
| `poc_dedup/` | ~152k | Step 6 — ID dedup + semantic similarity dedup | **Dedup set**, broad coverage without redundancy |
| `poc_gold_15/` | ~1.8k | Step 7 — score ≥15 | **Ultra-tight gold**, lowest false positive rate |
| `poc_gold_14/` | ~2.9k | Step 7 — score ≥14 | Gold |
| `poc_gold_13/` | ~45k | Step 7 — score ≥13 | **High-quality gold** |
| `poc_gold_12/` | ~94k | Step 7 — score ≥12 | **Standard gold**, balanced quality/coverage |
| `poc_gold_11/` | ~143k | Step 7 — score ≥11 | **Baseline gold**, wide coverage |
| `poc_excluded/` | ~550 | Step 3 — intercepted non-nuclei files | Kept for audit, not fed into the pipeline |

Containment: `poc_gold_15/ ⊂ poc_gold_14/ ⊂ poc_gold_13/ ⊂ poc_gold_12/ ⊂ poc_gold_11/ ⊂ poc_dedup/ ⊂ poc/`

> **Which to use?** Daily scanning → `poc/`. Maximum detection rate → `poc_dedup/`. Publishing or baselining → `poc_gold/`.

---

## Quick Start

```bash
git clone https://github.com/adysec/nuclei_poc
cd nuclei_poc

# Scan a target
nuclei -t poc/ -u http://example.com              # Full scan
nuclei -t poc_dedup/ -u http://example.com        # Dedup set (high detection)
nuclei -t poc_gold_11/ -u http://example.com      # Baseline gold (wide coverage)
nuclei -t poc_gold_12/ -u http://example.com      # Standard gold
nuclei -t poc_gold_13/ -u http://example.com      # High-quality
nuclei -t poc_gold_15/ -u http://example.com      # Ultra-tight (lowest FPs)
nuclei -t poc/cve/ -u http://example.com          # By category
```

---

## Pipeline

Runs every 2 hours. 9 stages total:

| # | Binary | Purpose |
|---|--------|---------|
| 1 | `1_clone_repos` | Clone upstream repos from `repo.csv`, then re-inject existing poc dirs for cyclic processing |
| 2 | `2_delete_duplicated` | SHA256 exact dedup (first pass — removes byte-identical files) |
| 3 | `3_move_file` | Filter non-nuclei files → `poc_excluded/`, categorize the rest into `tmp/` |
| 4 | `4_check_poc` | `auto_fix_poc()` repair → nuclei validate → pass → `poc/`, fail → `poc_needs_review/` |
| 5 | `5_dedup_advanced` | ID dedup + cross-ID multi-factor semantic similarity dedup + format repair → `poc_dedup/` |
| 6 | `6_dedup_high_quality` | Quality scoring by tiers (default 11/12/13/14/15) → `poc_gold_{N}/` directories |
| 7 | `7_generate_browser_index` | Generate chunked JSON index files for GitHub Pages browser viewer |

> Note: Step 6: `--threshold` (similarity, default 70). Step 7: `--tiers` (gradient, default 11,12,13,14,15).

### Build & Run

```bash
# Build all binaries
cargo build --release

# Run individual stages
./target/release/1_clone_repos --skip-clone          # Re-inject existing poc dirs, skip upstream clone
./target/release/7_dedup_high_quality --tiers 10,13,16  # Custom tier thresholds
./target/release/6_dedup_advanced --threshold 80         # Raise similarity threshold

# Dev / debug
cargo run --bin 4_check_poc -- --nuclei-bin ./nuclei --jobs 8
```

---

## Directory Layout

```
nuclei_poc/
├── Cargo.toml              # Rust project config
├── repo.csv                # Upstream repo list (extensible)
├── .github/workflows/       # GitHub Actions CI
│
├── src/
│   ├── core/               # Shared lib: hash, yaml, category, naming, features, index
│   └── bin/                # 8 standalone binaries, one per pipeline stage
│
├── poc/                    # ✅ Nuclei-validated PoCs (categorized)
├── poc_dedup/              # 📦 Step 6 dedup: semantic dedup + format repair (~152k)
├── poc_gold_11/       # ⭐ Gold (score ≥11, ~143k)
├── poc_gold_12/       # ⭐ Gold (score ≥12, ~94k)
├── poc_gold_13/       # ⭐⭐ Gold (score ≥13, ~45k)
├── poc_gold_14/       # ⭐⭐ Gold (score ≥14, ~2.9k)
├── poc_gold_15/       # ⭐⭐⭐ Gold (score ≥15, ~1.8k)
├── poc_excluded/           # 🚫 Non-nuclei files (audit trail)
├── poc_needs_review/       # ⚠️ Failed nuclei validation (manual review)
├── poc_all/                # 📚 Full archive (historical rollback)
```

---

## Deduplication Strategy

The project uses a **dedup-first, then extract multi-tier gold** pipeline:

```
poc/  ──Step 6──▶  poc_dedup/  ──Step 7──▶  poc_gold_11/ ──▶ poc_gold_12/ ──▶ poc_gold_13/ ──▶ poc_gold_14/ ──▶ poc_gold_15/
                   (~152k)        (~143k)     (~94k)      (~45k)      (~2.9k)    (~1.8k)
```

Higher tiers are subsets of lower tiers: `gold_15 ⊂ gold_14 ⊂ gold_13 ⊂ gold_12 ⊂ gold_11 ⊂ dedup ⊂ poc`.

### Stage 1: Step 6 → `poc_dedup/` (Full Dedup Set)

| Strategy | Detail |
|----------|--------|
| ID dedup | Among PoCs sharing the same `id`, keep only the highest-quality one |
| Semantic similarity dedup | Cross-ID comparison over 18 factors, score ≥70 → considered duplicate |
| Format repair | Auto-fix severity casing, empty severity, ID whitespace, etc. |
| File renaming | Normalize names by CVE/CNVD/protocol/path |

**Positioning**: Full-coverage dedup set, retaining ~24% of PoCs. Broad coverage, zero redundancy.

### Stage 2: Step 7 → `poc_gold_{N}/` (Multi-Tier Gold)

Single pass over `poc_dedup/` produces multiple quality tiers. Each tier
is independently SHA256-deduplicated.

| Tier | Threshold | Typical count | Use case |
|------|-----------|---------------|----------|
| `poc_gold_11/` | ≥11 | ~143k | Baseline quality, wide coverage (94% of dedup) |
| `poc_gold_12/` | ≥12 | ~94k | Standard gold, balanced quality/coverage |
| `poc_gold_13/` | ≥13 | ~45k | High quality (30% of dedup) |
| `poc_gold_14/` | ≥14 | ~2.9k | Very high quality |
| `poc_gold_15/` | ≥15 | ~1.8k | Ultra-tight, lowest false positive rate |

| Strategy | Detail |
|----------|--------|
| Quality scoring | Weighted scoring on `id/info/requests/matchers/severity` (max ~32) |
| SHA256 exact dedup | Byte-identical files kept only once per tier |
| Severity whitelist | Only `critical/high/medium` allowed |
| Auto-fix | `auto_fix_poc()` applied before scoring |

**Positioning**: High-barrier gold set, retaining ~15.7% of PoCs (~62% of dedup). Suitable for publishing and security baseline benchmarking.

### Step 7 Scoring Factors (0–80 points, 18 items)

| Category | Factors | Points |
|----------|---------|--------|
| Basic structure | `id`, `name`, `severity` | 0–7 |
| Severity level | critical=8, high=6, medium=4, low=2, info=1 | 0–8 |
| Protocol support | http+matchers=6, requests+matchers=5, tcp/dns=3 | 0–10 |
| Metadata | author/description/tags/reference/classification/remediation | 0–16 |
| Detection capability | matchers=5, extractors=4, URL count ≤6 | 0–15 |
| Vulnerability association | CVE=6, CNVD=4 | 0–10 |
| Format normalization | http (not requests)=3, severity casing=1, no deprecated network=2 | 0–6 |
| File size | 500B–10KB=5, 200–20KB=3, <200=1 | 0–5 |
| Multi-protocol | 2+ protocols = 3 | 0–3 |

---

## PR Quality Gate

When a PR touches files under `poc/`, `poc_dedup/`, or `tmp/`, `10_pr_check` is triggered:

- **YAML structure check** — verifies `id`/`info`/protocol fields
- **nuclei validate** — runs the nuclei engine to validate template legality
- **Weak matcher detection** — flags overly broad matcher words like `word`/`matchers`
- **Honeypot check** (opt-in) — detects templates referencing known honeypot URLs

---

## GitHub Actions

The pipeline is configured to run every 2 hours automatically. For use in your own fork:

> Set `Settings → Actions → General → Workflow permissions` to **Read and write**.

Add or modify upstream repositories in `repo.csv` to extend the collection scope.

---

## Acknowledgements

- [ProjectDiscovery](https://github.com/projectdiscovery/nuclei) — the Nuclei engine and open-source community
- [TajangSec](https://github.com/TajangSec) — code optimizations and improvement suggestions
- [重剑无锋](https://github.com/TideSec) — deduplication rule optimization suggestions

