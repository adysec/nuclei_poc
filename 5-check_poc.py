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
        print(f"poc校验失败，已删除文件: {file_path}")

def move_file(src, dest):
    os.makedirs(os.path.dirname(dest), exist_ok=True)
    try:
        if os.path.exists(dest):
            base, ext = os.path.splitext(dest)
            counter = 1
            new_dest = f"{base}_{counter}{ext}"
            while os.path.exists(new_dest):
                counter += 1
                new_dest = f"{base}_{counter}{ext}"
            dest = new_dest
        shutil.move(src, dest)
        print(f"poc校验成功，已移动文件: {src} -> {dest}")
    except Exception as e:
        print(f"移动文件出错: {src} -> {dest}, 错误: {e}")

def check_yaml_format(file_path):
    result = subprocess.run(["./nuclei", "-t", file_path, "-silent"],
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    return "FTL" not in result.stdout and "FTL" not in result.stderr

ensure_dir(POC_DIR)
if not os.path.exists(TMP_DIR):
    print("tmp/ 目录不存在，退出。")
    exit(1)

yaml_files = [os.path.join(root, file) for root, _, files in os.walk(TMP_DIR)
              for file in files if file.endswith(('.yml', '.yaml'))]
if not yaml_files:
    shutil.rmtree(TMP_DIR, ignore_errors=True)
    print("tmp/ 目录已删除。")
    exit(0)

for file_path in yaml_files:
    if time.time() - START_TIME >= TIMEOUT:
        print("运行时间已超过 5 小时 30 分钟，强制退出。")
        exit(0)

    if not check_yaml_format(file_path):
        safe_remove(file_path)
    else:
        move_file(file_path, os.path.join(POC_DIR, os.path.relpath(file_path, TMP_DIR)))

if not any(os.scandir(TMP_DIR)):
    shutil.rmtree(TMP_DIR, ignore_errors=True)
    print("tmp/ 目录已删除。")

print("POC 检查完成。")
