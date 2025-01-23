import os
import hashlib

def calculate_file_hash(file_path):
    """计算文件的SHA-256哈希值"""
    hash_sha256 = hashlib.sha256()
    with open(file_path, 'rb') as f:
        for chunk in iter(lambda: f.read(4096), b""):
            hash_sha256.update(chunk)
    return hash_sha256.hexdigest()

def count_files_in_categories(base_dir):
    category_counts = {}
    unique_hashes = set()

    # 遍历输出目录中的所有分类目录
    for category in os.listdir(base_dir):
        category_path = os.path.join(base_dir, category)
        if os.path.isdir(category_path):
            file_count = 0
            
            # 遍历分类目录中的文件
            for f in os.listdir(category_path):
                file_path = os.path.join(category_path, f)
                if os.path.isfile(file_path):
                    file_count += 1
                    # 计算文件哈希并加入集合（用于去重）
                    file_hash = calculate_file_hash(file_path)
                    unique_hashes.add(file_hash)

            category_counts[category] = file_count

    return category_counts, unique_hashes

# 输出目录路径
output_path = "poc"

# 调用统计函数
category_counts, unique_hashes = count_files_in_categories(output_path)

# 打印统计结果
print("各类文件数量:")
total_files = 0
for category, count in category_counts.items():
    print(f"{category}: {count}")
    total_files += count

print(f"总文件数量: {total_files}")

# 打印去重后文件数量
unique_file_count = len(unique_hashes)
print(f"去重后文件数量: {unique_file_count}")
