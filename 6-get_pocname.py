import os

os.system('find . -type f \( -iname "*.yaml" -o -iname "*.yml" \)| sort > poc.txt')
