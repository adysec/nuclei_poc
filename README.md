# Nuclei POCs

<a href="https://github.com/adysec/nuclei_poc/stargazers"><img alt="GitHub Repo stars" src="https://img.shields.io/github/stars/adysec/nuclei_poc?color=yellow&logo=riseup&logoColor=yellow&style=flat-square"></a>
<a href="https://github.com/adysec/nuclei_poc/network/members"><img alt="GitHub forks" src="https://img.shields.io/github/forks/adysec/nuclei_poc?color=orange&style=flat-square"></a>
<a href="https://github.com/adysec/nuclei_poc/issues"><img alt="GitHub issues" src="https://img.shields.io/github/issues/adysec/nuclei_poc?color=red&style=flat-square"></a>

Nuclei POC，每日更新

[中文](https://github.com/adysec/nuclei_poc/blob/main/README.md) | [English](https://github.com/adysec/nuclei_poc/blob/main/README_EN.md)

该项目已完成由 Python 向 Rust 的迁移与重写，在处理大规模仓库与 PoC 验证任务时速度显著提升（在Github Action上，从旧版约 6 小时缩短至约 6 分钟）。

迁移完成后，项目已优化 PoC 的格式验证与去重逻辑。为避免在逻辑演进过程中发生 PoC 丢失，并便于对比与回溯，目前采用灰度策略同时保留两套输出目录。

## 如何使用

克隆项目并进入目录

```bash
git clone https://github.com/adysec/nuclei_poc
cd nuclei_poc
```

使用 nuclei 调用 poc 扫描站点

```bash
./nuclei -t poc/ -u http://example.com
# 只扫描部分poc
./nuclei -t poc/web/ -u http://example.com
./nuclei -t poc/wordpress/ -u http://example.com
```

### 配置

在 `repo.csv` 文件中配置监控 GitHub 项目信息。迁移至 Rust 后，核心流水线以独立的 Rust 可执行文件分段实现，所有源文件位于 `src/bin/`。

### 构建与运行（Rust）

推荐在目标环境中构建 release 二进制以获得最佳性能：

```bash
# 构建 release 可执行文件
cargo build --release

# 运行示例（运行单个流水线任务）
./target/release/1_clone_repos

# 或使用 cargo 在开发模式下运行（便于调试）
cargo run --bin 1_clone_repos
```

生产环境建议使用 `--release` 构建并在 CI 或容器中以并发/资源限制方式运行，以达到 `≈6 分钟` 的处理时长（具体取决于并发配置与机器性能）。

### GitHub Action

在 GitHub 仓库中设置 Action，以便每日自动运行脚本。

> 需要配置`Workflow permissions`为`Read and write`权限

## 项目结构（整体梳理）

### 根目录（常见文件/目录）

- `Cargo.toml` — Rust 项目依赖与配置
- `run_all.sh` — 构建并顺序执行全部流水线二进制（见下方运行说明）
- `repo.csv` — 监控/采集的 GitHub 仓库列表（输入来源）
- `poc_all/` — 全量 PoC 输出目录（保留历史/完整产物）
- `poc_high_quality/` — 灰度策略输出：经过新去重/高质量筛选后的 PoC
- `poc/` — 按类别组织的 PoC 目录（用于 nuclei 等工具直接引用）
- `poc.txt` — 当前已归档 PoC 的列表（文本清单）
- `src/bin/` — 核心 Rust 源文件（每个文件对应一个可执行的流水线阶段）
- `target/` — cargo 构建输出（包含 release 可执行文件）

### 流水线

仓库中每个 `src/bin/<n>_<name>.rs` 都实现了流水线的一段逻辑：

1. 1_clone_repos — 批量克隆或更新 `repo.csv` 中列出的 GitHub 项目。
2. 2_delete_duplicated — 执行第一轮去重，删除明显重复的 PoC 文件。
3. 3_move_file — 将整理好的 PoC 文件归档到中间目录并按类别组织。
4. 4_download_nuclei — 下载/准备 Nuclei 引擎（若需要）以便后续验证。
5. 5_check_poc — 校验 PoC 是否有效并将有效项移动到输出目录（`poc/` 或其他指定目录）。
6. 6_get_count — 统计当前已归档 PoC 的数量并输出统计信息。
7. 7_get_pocname — 生成/更新 `poc.txt` 清单文件，便于快速引用与检索。
8. 8_dedup_high_quality — 新的去重与高质量筛选器（灰度逻辑），用于产出 `poc_high_quality/`。

### 输出策略与安全回滚

为了在升级去重/筛选逻辑时避免误删或丢失 PoC，当前采用灰度策略并同时输出两套结果：

- `poc_all/`：保持完整产物与历史记录，便于回溯与比对。
- `poc_high_quality/`：仅包含新逻辑筛选出的高质量 PoC，以便进行更严格的下游验证与发布。

在调试或单独运行某个阶段时，可直接使用 `cargo run --release --bin <bin_name>` 或运行 `target/release/<bin_name>`：

```bash
# 构建 release
cargo build --release

# 运行单个阶段（例如第 1 步）
./target/release/1_clone_repos

# 或使用 cargo 运行（开发/调试）
cargo run --bin 1_clone_repos -- <args...>
```

## 致谢

在本项目的开发过程中，得到了很多支持和帮助。在此特别感谢以下人员和项目：

### 项目

感谢 [ProjectDiscovery](https://github.com/projectdiscovery/nuclei) 提供的Nuclei工具和开源社区支持。

### 人员

感谢 [TajangSec](https://github.com/TajangSec) 对部分代码的优化和改进建议。
感谢 [重剑无锋](https://github.com/TideSec) 对去重规则的优化和改进建议。
