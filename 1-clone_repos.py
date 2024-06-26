import os

repo_file = "repo.csv"
clone_dir = "clone-templates"

# 确保克隆目录存在
try:
    os.makedirs(clone_dir, exist_ok=True)
except OSError as e:
    print(f"创建目录 {clone_dir} 时出错: {e}")
    exit(1)

# 读取并处理仓库文件中的URL
try:
    with open(repo_file, 'r') as file:
        urls = list(set(line.strip() for line in file if line.strip()))
except FileNotFoundError:
    print(f"文件 {repo_file} 未找到。")
    exit(1)
except Exception as e:
    print(f"读取文件 {repo_file} 时出错: {e}")
    exit(1)

for url in urls:
    parts = url.split('/')
    if len(parts) >= 2:
        owner, repo_name = parts[-2], parts[-1]
        target_dir = os.path.join(clone_dir, f"{owner}/{repo_name}".lower())
    else:
        print(f"无效的URL格式: {url}")
        continue

    if os.path.isdir(target_dir):
        print(f"更新 {repo_name} 在 {target_dir}")
        try:
            result = os.system(f"git -C {target_dir} pull")
            if result != 0:
                print(f"更新仓库 {repo_name} 在 {target_dir} 时出错")
        except Exception as e:
            print(f"更新仓库 {repo_name} 在 {target_dir} 时出错: {e}")
    else:
        print(f"克隆 {repo_name} 到 {target_dir}")
        try:
            result = os.system(f"git clone {url} {target_dir}")
            if result != 0:
                print(f"克隆仓库 {repo_name} 到 {target_dir} 时出错")
        except Exception as e:
            print(f"克隆仓库 {repo_name} 到 {target_dir} 时出错: {e}")
