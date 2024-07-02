import os
import yaml

POC_DIR = "poc"

def is_valid_poc(poc_file):
    try:
        with open(poc_file, 'r', encoding='utf-8') as f:
            poc_data = yaml.safe_load(f)
                return True
            else:
                return False
    except yaml.YAMLError as e:
        print(f"Error parsing YAML in {poc_file}: {e}")
        return False
    except IOError as e:
        print(f"Error reading file {poc_file}: {e}")
        return False

def check_and_delete_invalid_pocs():
    for filename in os.listdir(POC_DIR):
        if filename.endswith(".yml") or filename.endswith(".yaml"):
            poc_path = os.path.join(POC_DIR, filename)
            print(f"Checking {filename} ...")
            if not is_valid_poc(poc_path):
                print(f"{filename} is not valid. Deleting...")
                try:
                    os.remove(poc_path)
                    print(f"{filename} deleted.")
                except OSError as e:
                    print(f"Error deleting {filename}: {e}")
            else:
                print(f"{filename} is valid.")

if __name__ == "__main__":
    check_and_delete_invalid_pocs()
