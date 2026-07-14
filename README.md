# Nuclei POCs

<a href="https://github.com/adysec/nuclei_poc/stargazers"><img alt="GitHub Repo stars" src="https://img.shields.io/github/stars/adysec/nuclei_poc?color=yellow&logo=riseup&logoColor=yellow&style=flat-square"></a>
<a href="https://github.com/adysec/nuclei_poc/network/members"><img alt="GitHub forks" src="https://img.shields.io/github/forks/adysec/nuclei_poc?color=orange&style=flat-square"></a>
<a href="https://github.com/adysec/nuclei_poc/issues"><img alt="GitHub issues" src="https://img.shields.io/github/issues/adysec/nuclei_poc?color=red&style=flat-square"></a>

Nuclei POC，每日更新

[中文](https://github.com/adysec/nuclei_poc/blob/main/README.md) | [English](https://github.com/adysec/nuclei_poc/blob/main/README_EN.md)

该项目已完成由 Python 向 Rust 的迁移与重写，在处理大规模仓库与 PoC 验证任务时速度显著提升（在Github Action上，从旧版约 6 小时缩短至约 6 分钟）。

迁移完成后，项目已优化 PoC 的格式验证与去重逻辑。为避免在逻辑演进过程中发生 PoC 丢失，并便于对比与回溯，目前采用灰度策略同时保留两套输出目录。

## 如何使用

使用 nuclei 调用 poc 扫描站点

```bash
# 只下载高质量的poc
git clone --filter=tree:0 --sparse https://github.com/adysec/nuclei_poc
cd nuclei_poc
git sparse-checkout set poc_gold_13

# 只扫描部分poc
./nuclei -t poc_gold_13/ -u http://example.com
./nuclei -t poc_gold_13/web/ -u http://example.com
```

### 配置

在 `repo.csv` 文件中配置监控 GitHub 项目信息。迁移至 Rust 后，核心流水线以独立的 Rust 可执行文件分段实现，所有源文件位于 `src/bin/`。

## 项目结构（整体梳理）

### 流水线

仓库中每个 `src/bin/<n>_<name>.rs` 都实现了流水线的一段逻辑：

1. 1_clone_repos — 批量克隆或更新 `repo.csv` 中列出的 GitHub 项目。
2. 2_delete_duplicated — 执行第一轮去重，删除明显重复的 PoC 文件。
3. 3_move_file — 预过滤非 nuclei 文件（→ `poc_non_nuclei/`）后将 PoC 按类别归档到 `tmp/` 和 `poc_all/`。
4. 4_check_poc — 先 `auto_fix_poc()` 修复常见格式问题，再运行 nuclei 校验；通过→`poc/`，未通过→`poc_needs_review/`（不删除）。
5. 5_dedup_advanced — 多因素评分去重+格式修复。（读取 poc/ → 输出 poc_dedup/）
6. 6_dedup_high_quality — 多级评分梯度精选，产生 poc_gold_11 ~ poc_gold_15 目录。
7. 7_generate_browser_index — 生成 GitHub Pages 前端所需的 JSON 索引文件到 docs/。

### 根目录（常见文件/目录）

- `Cargo.toml` — Rust 项目依赖与配置
- `repo.csv` — 监控/采集的 GitHub 仓库列表（输入来源）
- `poc_non_nuclei/` — Step 3 拦截的非 nuclei 文件（docker-compose.yml 等），保留审计不删除
- `poc_needs_review/` — Step 4 nuclei 验证失败文件，保留供人工审核
- `poc_all/` — 全量 PoC 输出目录（保留历史/完整产物）
- `poc_baseline/` — 简单评分去重基线输出（第7步，用于对比）
- `poc_high_quality/` — 灰度策略输出：经过高级多因素去重+格式修复后的高质量 PoC（第8步）
- `poc/` — 按类别组织的 PoC 目录（用于 nuclei 等工具直接引用）
- `poc.txt` — 当前已归档 PoC 的列表（文本清单）
- `src/core/` — 共享公共库（哈希hash、YAML解析/验证/修复yaml、分类映射category、命名规范naming、特征提取features、JSON索引index）
- `src/bin/` — 核心 Rust 源文件（每个文件对应一个可执行的流水线阶段）

### 输出策略与安全回滚

为了在升级去重/筛选逻辑时避免误删或丢失 PoC，当前采用分层策略：

- `poc_all/`：全量原始归档（已通过 nuclei 结构预过滤），便于回溯与比对。
- `poc_non_nuclei/`：Step 3 拦截的非 nuclei 文件（如 docker-compose.yml 等），保留以备审计，不进入后续管线。
- `poc_needs_review/`：Step 4 nuclei 验证未通过的文件，保留供人工审核——不直接删除。
- `poc/`：nuclei 校验通过且经过 auto-fix 的组织化 PoC，可直接用于扫描。
- `poc_dedup/`：Step 6 多因素去重+格式修复后的输出。
- `poc_gold_11 ~ poc_gold_15`：Step 7 多级评分精选，评分越高越精品。

**格式过滤策略（保守原则）**：
1. Step 3: `is_nuclei_template()` 快速结构检查 → 拦截明显非 nuclei 的 YAML 文件（无 `id` 字段、无协议字段）
2. Step 4: `auto_fix_poc()` 修复 severity 大小写/空值/CVE 空格 → 再运行 nuclei 验证 → 未通过则移入 `poc_needs_review/`
3. Step 6: CVE 匹配权重 30 分（不单独触发重复判定），同一 CVE 的不同产品/端点变体不会被误删

评分规则（0-80分，18因子）：
- 基础结构 (0-7): id, name, severity
- 严重程度 (0-8): critical=8, high=6, medium=4, low=2, info=1
- 协议支持 (0-10): http+matchers=6, requests+matchers=5, tcp/dns=3ea
- 元数据丰富度 (0-16): author/description/tags/reference/classification/remediation ×2
- 检测能力 (0-15): matchers=5, extractors=4, URL数(≤6)
- 漏洞关联 (0-10): CVE=6, CNVD=4
- 格式规范 (0-6): http(非requests)=3, 无severity大小写问题=1, 无废弃network=2
- 文件大小合理性 (0-5): 500B-10KB=5, 200-20KB=3, <200=1
- 多协议加分 (0-3): 2+协议=3

## 致谢

在本项目的开发过程中，得到了很多支持和帮助。在此特别感谢以下人员和项目：

### 项目

感谢 [ProjectDiscovery](https://github.com/projectdiscovery/nuclei) 提供的Nuclei工具和开源社区支持。

### 人员

感谢 [TajangSec](https://github.com/TajangSec) 对部分代码的优化和改进建议。

感谢 [重剑无锋](https://github.com/TideSec) 对去重规则的优化和改进建议。
