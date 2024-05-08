import os

os.system('rm -rf poc clone-templates')
os.system('python 1-clone_repos.py')
os.system('python 2-remove_duplicated.py')
os.system('python 2-1-remove_duplicated.py')
os.system('python 3-get_pocname.py')

