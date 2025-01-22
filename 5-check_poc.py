import os
import shutil
import subprocess
import time

POC_DIR, TMP_DIR, TIMEOUT, START_TIME = "poc", "tmp", 19800, time.time()

def ensure_dir(directory):
    os.makedirs(directory, exist_ok=True)

def safe_remove(file_path):
    if os.path.exists(file_path):
        os.remove(file_path)
        print(f"已删除 {file_path}")

def move_file(src, dest):
    os.makedirs(os.path.dirname(dest), exist_ok=True)
    shutil.move(src, dest)
    print(f"已将文件 {src} 移动至 {dest}")

def check_yaml_format(file_path):
    result = subprocess.run(["./nuclei", "-t", file_path, "-silent"],
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    return "FTL" not in result.stdout and "FTL" not in result.stderr

ensure_dir(POC_DIR)
if not os.path.exists(TMP_DIR):
    print("TMP 目录不存在，退出。")
    exit(1)

yaml_files = [os.path.join(root, file) for root, _, files in os.walk(TMP_DIR)
              for file in files if file.endswith(('.yml', '.yaml'))]
if not yaml_files:
    shutil.rmtree(TMP_DIR, ignore_errors=True)
    print("TMP 目录已删除。")
    exit(0)

for file_path in yaml_files:
    if time.time() - START_TIME >= TIMEOUT:
        print("运行时间已超过 5 小时 30 分钟，强制退出。")
        exit(0)

    if not check_yaml_format(file_path):
        print(f"检查 POC 格式无效，已删除 {file_path}")
        safe_remove(file_path)
    else:
        move_file(file_path, os.path.join(POC_DIR, os.path.relpath(file_path, TMP_DIR)))

if not any(os.scandir(TMP_DIR)):
    shutil.rmtree(TMP_DIR, ignore_errors=True)
    print("TMP 目录已删除。")

print("POC 检查完成。")
