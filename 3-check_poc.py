import os

POC_DIR = "clone-templates"

yaml_files = [f for f in os.listdir(POC_DIR) if f.endswith('.yaml') or f.endswith('.yml')]

for file in yaml_files:
	file_path = os.path.join(POC_DIR, file)
	print(f"检查POC {file_path} 中...")
	
	command = f"./nuclei -t {file_path} -silent"
	return_code = os.system(command)
	
	if return_code == 0:
		print(f"{file_path}格式有效")
	else:
		print(f"{file_path}格式无效，已删除")
		os.remove(file_path)
