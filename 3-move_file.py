import os
import shutil
import hashlib

# 分类映射，定义了各种分类与其对应的关键词
category_map = {
    "wordpress": ["wp", "wordpress"],
    "xss": ["xss"],
    "sql_injection": ["sqli", "sql_injection","sql"],
    "local_file_inclusion": ["lfi","local_file_inclusion"],
    "remote_code_execution": ["rce"],
    "cross_site_request_forgery": ["csrf"],
    "xml_external_entity": ["xxe"],
    "cve": ["cve"],
    "cnvd":["cnvd"],
    "cnnvd":["cnnvd"],
    "open_redirect": ["redirect", "open_redirect"],
    "ssrf": ["ssrf", "server_side_request_forgery"],
    "subdomain_takeover": ["subdomain_takeover", "takeover"],
    "template_injection": ["template_injection", "ssti"],
    "crlf_injection": ["crlf_injection", "crlf"],
    "directory_listing": ["directory_listing", "traversal"],
    "exposed": ["exposed", "disclosure", "sensitive", "exposure"],
    "adobe": ["adobe","aem",],
    "coldfusion": ["coldfusion", "cfm"],
    "drupal": ["drupal"],
    "joomla": ["joomla"],
    "magento": ["magento"],
    "php": ["php"],
    "airflow": ["airflow"],
    "aws": ["aws", "amazon", "ec2", "s3", "lambda", "cloudfront", "cloudfront"],
    "apache": ["apache"],
    "cpanel": ["cpanel"],
    "docker": ["docker", "container", "kubernetes"],
    "git": ["git"],
    "jenkins": ["jenkins"],
    "cisco": ["cisco"],
    "api": ["api"],
    "upload": ["upload"],
    "sensitive": ["sensitive"],
    "debug": ["debug"],
    "backup": ["backup"],
    "auth": ["auth","login","signin","sign_in","sign-in","oauth","sso","register","signup","sign_up","sign-up","password","pwd","passwd","secret","token","credential","cred","jwt","cookie","session","remember","keycloak","key",],
    "atlassian": ["atlassian", "jira", "confluence", "bitbucket", "bamboo"],
    "config": ["config", "conf", "configuration"],
    "mysql": ["mysql", "mariadb"],
    "sql": ["sql", "database", "db"],
    "default": ["default"],
    "detect": ["detect"],
    "extract": ["extract"],
    "fuzz": ["fuzz"],
    "graphql": ["graphql"],
    "http": ["http"],
    "social": ["social","social_media","facebook","twitter","instagram","linkedin",],
    "favicon": ["favicon"],
    "python": ["python", "flask", "django"],
    "ftp": ["ftp"],
    "gcloud": ["gcloud", "google_cloud", "gcp"],
    "google": ["google"],
    "graphite": ["graphite"],
    "header": ["header"],
    "injection": ["injection"],
    "ibm": ["ibm"],
    "search": ["search"],
    "ldap": ["ldap"],
    "microsoft": ["microsoft", "ms"],
    "mongodb": ["mongodb", "mongo"],
    "netlify": ["netlify"],
    "oracle": ["oracle"],
    "java": ["java","jsp","jsf","j2ee","j2se","j2me","jvm","jre","jdk","jboss","tomcat","glassfish","wildfly","jetty","websphere","weblogic","spring","struts","hibernate","mybatis","shiro",],
    "javascript": ["javascript", "js"],
    "elk": ["elk", "elasticsearch", "kibana", "logstash"],
    "kafka": ["kafka"],
    "kong": ["kong"],
    "laravel": ["laravel"],
    "nginx": ["nginx"],
    "nodejs": ["nodejs", "node", "express", "npm"],
    "perl": ["perl"],
    "postgres": ["postgres", "postgresql"],
    "rabbitmq": ["rabbitmq"],
    "redis": ["redis"],
    "ruby": ["ruby", "rails"],
    "samba": ["samba"],
    "sharepoint": ["sharepoint"],
    "smtp": ["smtp"],
    "sap": ["sap"],
    "shopify": ["shopify"],
    "ssh": ["ssh"],
    "vmware": ["vmware"],
    "web": ["web"],
}

# 获取目录下所有的yaml文件，返回文件名和路径的字典
def get_all_yaml_files(dir_path):
    all_yaml_files = {}
    for dirpath, dirs, files in os.walk(dir_path):
        dirs[:] = [d for d in dirs if d != ".git" and d != "projectdiscovery__nuclei-templates"]
        for filename in files:
            if filename.endswith(".yml") or filename.endswith(".yaml"):
                all_yaml_files[filename] = os.path.join(dirpath, filename)
    return all_yaml_files

# 根据文件名和关键词映射，分类文件
def categorize_file(file_name, category_map):
    categories = []
    for category, keywords in category_map.items():
        if any(keyword in file_name.lower() for keyword in keywords):
            categories.append(category)
    return categories if categories else ["other"]

# 计算文件的哈希值
def file_hash(file_path):
    hasher = hashlib.md5()
    with open(file_path, "rb") as f:
        buf = f.read()
        hasher.update(buf)
    return hasher.hexdigest()

# 将文件移动到对应分类目录下，避免重复文件
def copy_file_to_categories(file_path, base_dir, category_map, file_hashes):
    categories = categorize_file(os.path.basename(file_path), category_map)
    file_hash_value = file_hash(file_path)
    for category in categories:
        target_dir = os.path.join(base_dir, category)
        os.makedirs(target_dir, exist_ok=True)

        if file_hash_value not in file_hashes.get(category, set()):
            shutil.copy(file_path, os.path.join(target_dir, os.path.basename(file_path)))
            file_hashes.setdefault(category, set()).add(file_hash_value)

# 获取poc目录下所有文件的哈希值
def get_poc_file_hashes(poc_dir_path):
    poc_file_hashes = set()
    for dirpath, dirs, files in os.walk(poc_dir_path):
        for filename in files:
            if filename.endswith(".yml") or filename.endswith(".yaml"):
                file_path = os.path.join(dirpath, filename)
                poc_file_hashes.add(file_hash(file_path))
    return poc_file_hashes

# 在复制文件前，先通过哈希值与poc目录进行对比
def copy_file_if_unique(file_path, base_dir, category_map, file_hashes, poc_file_hashes):
    file_hash_value = file_hash(file_path)
    if file_hash_value not in poc_file_hashes:
        categories = categorize_file(os.path.basename(file_path), category_map)
        for category in categories:
            target_dir = os.path.join(base_dir, category)
            os.makedirs(target_dir, exist_ok=True)
            
            if file_hash_value not in file_hashes.get(category, set()):
                shutil.copy(file_path, os.path.join(target_dir, os.path.basename(file_path)))
                file_hashes.setdefault(category, set()).add(file_hash_value)

# 获取poc目录下的所有文件的哈希值
poc_file_hashes = get_poc_file_hashes('poc')

community_path = "clone-templates"
source_of_truth = "clone-templates/projectdiscovery/nuclei-templates"
output_path = "tmp"

# 获取所有社区模板和模板源
community = get_all_yaml_files(community_path)
nucleiTemplates = get_all_yaml_files(source_of_truth)

# 获取共同的模板
common_templates = set(community.keys()) & set(nucleiTemplates.keys())
file_hashes = {}

# 遍历社区模板，进行处理
for template, community_file in community.items():
    if template in common_templates and os.path.getsize(community_file) == os.path.getsize(nucleiTemplates[template]):
        os.remove(community_file)
        continue
    # 在复制文件前，先通过哈希值与poc目录进行对比
    copy_file_if_unique(community_file, output_path, category_map, file_hashes, poc_file_hashes)

os.system('rm -rf clone-templates')
