import os
import subprocess

POC_DIR = "clone-templates"

yaml_files = [f for f in os.listdir(POC_DIR) if f.endswith('.yaml') or f.endswith('.yml')]

for file in yaml_files:
    file_path = os.path.join(POC_DIR, file)
    print(f"检查 POC {file_path} 中...")

    # =捕获输出
    command = ["./nuclei", "-t", file_path, "-silent"]
    try:
        result = subprocess.run(command, capture_output=True, text=True)
        if result.returncode == 0 and not result.stderr.strip():
            print(f"{file_path} 格式有效")
        else:
            print(f"{file_path} 格式无效，已删除")
            os.remove(file_path)
    except Exception as e:
        print(f"执行命令时发生错误: {e}")
        print(f"{file_path} 格式无效，已删除")
        os.remove(file_path)
