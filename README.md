# Nuclei POCs
<a href="https://github.com/adysec/nuclei_poc/stargazers"><img alt="GitHub Repo stars" src="https://img.shields.io/github/stars/adysec/nuclei_poc?color=yellow&logo=riseup&logoColor=yellow&style=flat-square"></a>
<a href="https://github.com/adysec/nuclei_poc/network/members"><img alt="GitHub forks" src="https://img.shields.io/github/forks/adysec/nuclei_poc?color=orange&style=flat-square"></a>
<a href="https://github.com/adysec/nuclei_poc/issues"><img alt="GitHub issues" src="https://img.shields.io/github/issues/adysec/nuclei_poc?color=red&style=flat-square"></a>

Nuclei POC，每日更新

这个项目是一个 Python 脚本，用于批量克隆 GitHub 项目，获取 Nuclei POC，并将 POC 按类别分类存放到文件夹中。同时，使用 GitHub Action 每日自动运行脚本。

## 如何使用

### 克隆项目

克隆这个项目到本地：

```bash
git clone https://github.com/adysec/nuclei_poc.git
```

进入项目目录：

```bash
cd nuclei_poc
```

### 配置

在 `repo.csv` 文件中配置监控 GitHub 项目信息。

### 运行脚本

运行 Python 脚本：

```bash
python main.py
```

### GitHub Action

在 GitHub 仓库中设置 Action，以便每日自动运行脚本。

> 需要配置`Workflow permissions`为`Read and write`权限

## 文件结构

- `main.py`: 批量运行脚本文件。
- `1-clone_repos.py`: 批量克隆监控的 GitHub 项目。
- `4-remove_duplicated.py`: 删除重复POC脚本，并归档到分类文件夹。
- `5-get_pocname.py`: 读取并将POC列表写入`poc.txt`。
- `repo.csv`: Nuclei POC仓库列表。
- `poc.txt`: 已存档POC列表。
- `poc/`: 存放分类后的 Nuclei POC 文件夹。

