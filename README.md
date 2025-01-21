# Nuclei POCs
<a href="https://github.com/adysec/nuclei_poc/stargazers"><img alt="GitHub Repo stars" src="https://img.shields.io/github/stars/adysec/nuclei_poc?color=yellow&logo=riseup&logoColor=yellow&style=flat-square"></a>
<a href="https://github.com/adysec/nuclei_poc/network/members"><img alt="GitHub forks" src="https://img.shields.io/github/forks/adysec/nuclei_poc?color=orange&style=flat-square"></a>
<a href="https://github.com/adysec/nuclei_poc/issues"><img alt="GitHub issues" src="https://img.shields.io/github/issues/adysec/nuclei_poc?color=red&style=flat-square"></a>

Nuclei POC，每日更新

这个项目是一个 Python 脚本，用于批量克隆 GitHub 项目，获取 Nuclei POC，并将 POC 按类别分类存放到文件夹中。同时，使用 GitHub Action 每日自动运行脚本。

## 如何使用

解压poc.zip文件

```bash
wget https://raw.githubusercontent.com/adysec/nuclei_poc/refs/heads/main/poc.zip
unzip poc.zip
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
- `2-download_nuclei.py`: 下载nuclei以便验证POC有效性。
- `3-delete_duplicated.py`: 删除重复POC脚本。
- `4-move_file.py`: POC脚本归档。
- `5-get_count.py`: 获取POC脚本数量。
- `6-get_pocname.py`: 读取并将POC列表写入`poc.txt`。
- `check_poc.sh`: 验证POC有效性并打包为`poc.zip`文件。
- `repo.csv`: Nuclei POC仓库列表。
- `poc.txt`: 已存档POC列表。
- `poc/`: 存放分类后的 Nuclei POC 文件夹(未完全验证有效性)。
- `poc.zip`:: 已验证有效性 Nuclei POC 压缩文件。

