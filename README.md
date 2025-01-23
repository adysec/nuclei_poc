# Nuclei POCs

<a href="https://github.com/adysec/nuclei_poc/stargazers"><img alt="GitHub Repo stars" src="https://img.shields.io/github/stars/adysec/nuclei_poc?color=yellow&logo=riseup&logoColor=yellow&style=flat-square"></a>
<a href="https://github.com/adysec/nuclei_poc/network/members"><img alt="GitHub forks" src="https://img.shields.io/github/forks/adysec/nuclei_poc?color=orange&style=flat-square"></a>
<a href="https://github.com/adysec/nuclei_poc/issues"><img alt="GitHub issues" src="https://img.shields.io/github/issues/adysec/nuclei_poc?color=red&style=flat-square"></a>

Nuclei POC，每日更新

[中文](https://github.com/adysec/nuclei_poc/blob/main/README.md) | [English](https://github.com/adysec/nuclei_poc/blob/main/README_EN.md)

这个项目是一个 Python 脚本，用于批量克隆 GitHub 项目，获取 Nuclei POC，并将 POC 按类别分类存放到文件夹中。同时，使用 GitHub Action 每日自动运行脚本。

已更新优化poc格式验证相关代码，**当本项目中 `tmp/` 目录不存在时，所有poc格式校验完成**。

实际校验格式后，仅剩余 11.7w 可用 PoC 脚本，其中去重后的独立可用 PoC 脚本数量为 11.1w。原先计算的 14w+ 数量存在错误，特此修正。

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

在 `repo.csv` 文件中配置监控 GitHub 项目信息。

### GitHub Action

在 GitHub 仓库中设置 Action，以便每日自动运行脚本。

> 需要配置`Workflow permissions`为`Read and write`权限

## 文件结构

- `1-clone_repos.py`: 批量克隆监控的 GitHub 项目。
- `2-delete_duplicated.py`: 删除重复Poc脚本。
- `3-move_file.py`: Poc脚本归档至tmp目录。
- `4-download_nuclei.py`: 下载nuclei以便验证Poc有效性。
- `5-check_poc.sh`: 校验Poc有效性并移动至`poc`目录下。
- `6-get_count.py`: 获取已归档Poc数量。
- `7-get_pocname.py`: 读取并将Poc列表写入`poc.txt`。
- `check_poc.sh`: 验证Poc有效性并打包为`poc.zip`文件。
- `repo.csv`: Nuclei Poc仓库列表。
- `poc.txt`: 已存档Poc列表。
- `poc/`: 存放分类后的 Nuclei Poc 文件夹。
- ~~`clone-templates/`: 克隆 GitHub 项目的临时文件夹。~~
- ~~`tmp/`: Nuclei Poc脚本去重并分类后的临时文件夹。~~

## 致谢

在本项目的开发过程中，得到了很多支持和帮助。在此特别感谢以下人员和项目：

### 项目

感谢 [ProjectDiscovery](https://github.com/projectdiscovery/nuclei) 提供的Nuclei工具和开源社区支持。

### 人员

感谢 [TajangSec](https://github.com/TajangSec) 对部分代码的优化和改进建议。
