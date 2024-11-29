import os

os.system('rm -rf poc clone-templates')
os.system('python 1-clone_repos.py')
os.system('python 3-check_poc.py')
os.system('python 4-remove_duplicated.py')
os.system('python 5-get_pocname.py')

