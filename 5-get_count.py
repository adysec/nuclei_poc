import os

def count_files_in_categories(base_dir):
    category_counts = {}

    # 遍历输出目录中的所有分类目录
    for category in os.listdir(base_dir):
        category_path = os.path.join(base_dir, category)
        if os.path.isdir(category_path):
            # 统计分类目录中的文件数量
            file_count = len([f for f in os.listdir(category_path) if os.path.isfile(os.path.join(category_path, f))])
            category_counts[category] = file_count

    return category_counts

# 输出目录路径
output_path = "poc"

# 调用统计函数
category_counts = count_files_in_categories(output_path)

# 打印统计结果
print("各类文件数量:")
total_files = 0
for category, count in category_counts.items():
    print(f"{category}: {count}")
    total_files += count

print(f"总文件数量: {total_files}")
