import os
import hashlib

def file_hash(file_path):
    """计算文件的 MD5 哈希值"""
    with open(file_path, 'rb') as f:
        file_content = f.read()
        return hashlib.md5(file_content).hexdigest()

def deduplicate_files(root_dir):
    """去重文件"""
    file_hashes = {}
    for dirpath, dirnames, filenames in os.walk(root_dir):
        for filename in filenames:
            if filename.endswith(('.yml', '.yaml')):
                file_path = os.path.join(dirpath, filename)
                file_hash_value = file_hash(file_path)
                if file_hash_value not in file_hashes:
                    file_hashes[file_hash_value] = file_path
                else:
                    os.remove(file_path)
                    print(f"Removed duplicate file: {file_path}")

if __name__ == '__main__':
    root_dir = 'poc'
    deduplicate_files(root_dir)
