import os
import hashlib

# 计算文件的SHA256哈希值
def calculate_file_hash(filepath):
    hash_sha256 = hashlib.sha256()
    with open(filepath, "rb") as f:
        for chunk in iter(lambda: f.read(4096), b""):
            hash_sha256.update(chunk)
    return hash_sha256.hexdigest()

# 递归遍历目录，查找并删除重复的yml/yaml文件
def find_and_remove_duplicates(directory):
    hash_map = {}
    for root, _, files in os.walk(directory):
        for file in files:
            if file.endswith(('.yml', '.yaml')):
                filepath = os.path.join(root, file)
                file_hash = calculate_file_hash(filepath)

                if file_hash in hash_map:
                    print(f"删除重复文件: {filepath}")
                    os.remove(filepath)
                else:
                    hash_map[file_hash] = filepath

if __name__ == "__main__":
    base_directory = "clone-templates"  # 设置目标目录
    find_and_remove_duplicates(base_directory)
