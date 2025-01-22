#!/bin/bash

# 设置变量
POC_DIR="poc"
TMP_DIR="tmp"
TIMEOUT=19800  # 设置超时时间为5小时30分钟
START_TIME=$(date +%s)

# 检查 POC 目录是否存在，如果不存在则创建
if [ ! -d "$POC_DIR" ]; then
  echo "POC 目录不存在，创建中..."
  mkdir -p "$POC_DIR" || { echo "无法创建 POC 目录，退出。"; exit 1; }
  echo "POC 目录已创建。"
fi

# 检查 TMP 目录是否存在
if [ ! -d "$TMP_DIR" ]; then
  echo "TMP 目录不存在，退出。"
  exit 1
fi

# 查找 YAML 文件
find "$TMP_DIR" -type f \( -name "*.yml" -o -name "*.yaml" \) | while IFS= read -r file; do
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
    # 获取相对路径并创建目标目录
    dest_file="$POC_DIR/$(echo "$file" | sed "s|$TMP_DIR/||")"
    dest_dir=$(dirname "$dest_file")

    # 创建目标目录（如果不存在）
    if [ ! -d "$dest_dir" ]; then
      echo "目标目录 $dest_dir 不存在，创建中..."
      mkdir -p "$dest_dir" || { echo "无法创建目标目录，跳过 $file"; continue; }
    fi

    # 移动文件
    mv "$file" "$dest_file" && echo "已将文件 $file 移动至 $dest_file"
  fi
done

echo "POC 检查完成。"
