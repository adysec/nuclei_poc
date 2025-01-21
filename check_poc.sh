#!/bin/bash

# 设置变量
POC_DIR="poc"
TIMEOUT=19800  # 设置超时时间为5小时30分钟
START_TIME=$(date +%s)

# 检查 POC 目录是否存在
if [ ! -d "$POC_DIR" ]; then
  echo "POC 目录不存在，退出。"
  exit 1
fi

# 查找 YAML 文件
yaml_files=$(find "$POC_DIR" -type f \( -name "*.yml" -o -name "*.yaml" \))
if [ -z "$yaml_files" ]; then
  echo "未找到 YAML 文件，退出。"
  exit 0
fi

# 遍历每个 YAML 文件
for file in $yaml_files; do
  CURRENT_TIME=$(date +%s)
  ELAPSED_TIME=$((CURRENT_TIME - START_TIME))
  
  # 检查是否超时
  if [ "$ELAPSED_TIME" -ge "$TIMEOUT" ]; then
    echo "运行时间已超过 5 小时 30 分钟，强制退出。"
    exit 0
  fi

  # 检查 YAML 文件格式
  result=$(./nuclei -t "$file" -silent 2>&1)
  if echo "$result" | grep -q "FTL"; then
    echo "检查 POC 格式无效，已删除 $file"
    rm -f "$file" || echo "无法删除 $file，跳过。"
  else
    echo "检查 POC 格式有效 $file"
  fi
done

# 打包 POC 文件夹
echo "POC 检查完成，开始打包。"
zip -r poc.zip "$POC_DIR" > /dev/null 2>&1
if [ $? -eq 0 ]; then
  echo "POC 文件夹已成功打包为 poc.zip"
else
  echo "POC 文件夹打包失败。"
fi
