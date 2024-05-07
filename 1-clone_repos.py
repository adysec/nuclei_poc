import os
import subprocess

repo_file = "repo.csv"
clone_dir = "clone-templates"

os.makedirs(clone_dir, exist_ok=True)

with open(repo_file, 'r') as file:
	urls = list(set(line.strip() for line in file if line.strip()))

for url in urls:
	parts = url.split('/')
	if len(parts) >= 2:
		owner, repo_name = parts[-2], parts[-1]
		target_dir = os.path.join(clone_dir, f"{owner}__{repo_name}".lower())
	else:
		continue

	if os.path.isdir(target_dir):
		print(f"Updating {repo_name} in {target_dir}")
		subprocess.run(["git", "-C", target_dir, "pull"])
	else:
		print(f"Cloning {repo_name} into {target_dir}")
		subprocess.run(["git", "clone", url, target_dir])
