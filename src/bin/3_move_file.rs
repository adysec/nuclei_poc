use md5;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

fn category_map() -> HashMap<&'static str, Vec<&'static str>> {
    let mut m = HashMap::new();
    m.insert("wordpress", vec!["wp", "wordpress"]);
    m.insert("xss", vec!["xss"]);
    m.insert("sql_injection", vec!["sqli", "sql_injection", "sql"]);
    m.insert("local_file_inclusion", vec!["lfi", "local_file_inclusion"]);
    m.insert("remote_code_execution", vec!["rce"]);
    m.insert("cross_site_request_forgery", vec!["csrf"]);
    m.insert("xml_external_entity", vec!["xxe"]);
    m.insert("cve", vec!["cve"]);
    m.insert("cnvd", vec!["cnvd"]);
    m.insert("cnnvd", vec!["cnnvd"]);
    m.insert("open_redirect", vec!["redirect", "open_redirect"]);
    m.insert("ssrf", vec!["ssrf", "server_side_request_forgery"]);
    m.insert("subdomain_takeover", vec!["subdomain_takeover", "takeover"]);
    m.insert("template_injection", vec!["template_injection", "ssti"]);
    m.insert("crlf_injection", vec!["crlf_injection", "crlf"]);
    m.insert("directory_listing", vec!["directory_listing", "traversal"]);
    m.insert("exposed", vec!["exposed", "disclosure", "sensitive", "exposure"]);
    m.insert("adobe", vec!["adobe", "aem"]);
    m.insert("coldfusion", vec!["coldfusion", "cfm"]);
    m.insert("drupal", vec!["drupal"]);
    m.insert("joomla", vec!["joomla"]);
    m.insert("magento", vec!["magento"]);
    m.insert("php", vec!["php"]);
    m.insert("airflow", vec!["airflow"]);
    m.insert("aws", vec!["aws", "amazon", "ec2", "s3", "lambda", "cloudfront"]);
    m.insert("apache", vec!["apache"]);
    m.insert("cpanel", vec!["cpanel"]);
    m.insert("docker", vec!["docker", "container", "kubernetes"]);
    m.insert("git", vec!["git"]);
    m.insert("jenkins", vec!["jenkins"]);
    m.insert("cisco", vec!["cisco"]);
    m.insert("api", vec!["api"]);
    m.insert("upload", vec!["upload"]);
    m.insert("sensitive", vec!["sensitive"]);
    m.insert("debug", vec!["debug"]);
    m.insert("backup", vec!["backup"]);
    m.insert("auth", vec!["auth", "login", "signin", "sign_in", "sign-in", "oauth", "sso"]);
    m.insert("atlassian", vec!["atlassian", "jira", "confluence", "bitbucket", "bamboo"]);
    m.insert("config", vec!["config", "conf", "configuration"]);
    m.insert("mysql", vec!["mysql", "mariadb"]);
    m.insert("sql", vec!["sql", "database", "db"]);
    m.insert("default", vec!["default"]);
    m.insert("detect", vec!["detect"]);
    m.insert("extract", vec!["extract"]);
    m.insert("fuzz", vec!["fuzz"]);
    m.insert("graphql", vec!["graphql"]);
    m.insert("http", vec!["http"]);
    m.insert("social", vec!["social", "social_media", "facebook", "twitter", "instagram", "linkedin"]);
    m.insert("favicon", vec!["favicon"]);
    m.insert("python", vec!["python", "flask", "django"]);
    m.insert("ftp", vec!["ftp"]);
    m.insert("gcloud", vec!["gcloud", "google_cloud", "gcp"]);
    m.insert("google", vec!["google"]);
    m.insert("graphite", vec!["graphite"]);
    m.insert("header", vec!["header"]);
    m.insert("injection", vec!["injection"]);
    m.insert("ibm", vec!["ibm"]);
    m.insert("search", vec!["search"]);
    m.insert("ldap", vec!["ldap"]);
    m.insert("microsoft", vec!["microsoft", "ms"]);
    m.insert("mongodb", vec!["mongodb", "mongo"]);
    m.insert("netlify", vec!["netlify"]);
    m.insert("oracle", vec!["oracle"]);
    m.insert("java", vec!["java", "jsp", "jsf"]);
    m.insert("javascript", vec!["javascript", "js"]);
    m.insert("elk", vec!["elk", "elasticsearch", "kibana", "logstash"]);
    m.insert("kafka", vec!["kafka"]);
    m.insert("kong", vec!["kong"]);
    m.insert("laravel", vec!["laravel"]);
    m.insert("nginx", vec!["nginx"]);
    m.insert("nodejs", vec!["nodejs", "node", "express", "npm"]);
    m.insert("perl", vec!["perl"]);
    m.insert("postgres", vec!["postgres", "postgresql"]);
    m.insert("rabbitmq", vec!["rabbitmq"]);
    m.insert("redis", vec!["redis"]);
    m.insert("ruby", vec!["ruby", "rails"]);
    m.insert("samba", vec!["samba"]);
    m.insert("sharepoint", vec!["sharepoint"]);
    m.insert("smtp", vec!["smtp"]);
    m.insert("sap", vec!["sap"]);
    m.insert("shopify", vec!["shopify"]);
    m.insert("ssh", vec!["ssh"]);
    m.insert("vmware", vec!["vmware"]);
    m.insert("web", vec!["web"]);
    m
}

fn get_all_yaml_files(dir_path: &str) -> HashMap<String, PathBuf> {
    let mut result = HashMap::new();
    for entry in WalkDir::new(dir_path).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let filename = entry.file_name().to_string_lossy().into_owned();
            if filename.ends_with(".yml") || filename.ends_with(".yaml") {
                result.insert(filename, entry.path().to_path_buf());
            }
        }
    }
    result
}

fn categorize_file(file_name: &str, cmap: &HashMap<&str, Vec<&str>>) -> Vec<String> {
    let name = file_name.to_lowercase();
    let mut categories = vec![];
    for (cat, keywords) in cmap.iter() {
        if keywords.iter().any(|k| name.contains(k)) {
            categories.push((*cat).to_string());
        }
    }
    if categories.is_empty() {
        categories.push("other".to_string());
    }
    categories
}

// 计算文件 MD5 哈希
fn md5_hash_file(path: &Path) -> anyhow::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    Ok(format!("{:x}", md5::compute(&buf)))
}

// 获取已存在 POC 的哈希集合，用于去重
fn get_poc_file_hashes(poc_dir_path: &str) -> anyhow::Result<HashSet<String>> {
    let mut set = HashSet::new();
    for entry in WalkDir::new(poc_dir_path).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            let filename = entry.file_name().to_string_lossy().into_owned();
            if filename.ends_with(".yml") || filename.ends_with(".yaml") {
                let hash = md5_hash_file(entry.path())?;
                set.insert(hash);
            }
        }
    }
    Ok(set)
}

// 将唯一文件按类别拷贝到输出目录
fn copy_file_if_unique(
    file_path: &Path,
    base_dir: &str,
    cmap: &HashMap<&str, Vec<&str>>,
    file_hashes: &mut HashMap<String, HashSet<String>>,
    poc_file_hashes: &HashSet<String>,
) -> anyhow::Result<()> {
    let file_hash = md5_hash_file(file_path)?;
    if poc_file_hashes.contains(&file_hash) {
        return Ok(());
    }
    let categories = categorize_file(&file_path.file_name().unwrap().to_string_lossy(), cmap);
    for category in categories {
        let target_dir = Path::new(base_dir).join(&category);
        fs::create_dir_all(&target_dir)?;
        let set = file_hashes.entry(category.clone()).or_default();
        if !set.contains(&file_hash) {
            let dest = target_dir.join(file_path.file_name().unwrap());
            fs::copy(file_path, dest)?;
            set.insert(file_hash.clone());
        }
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let cmap = category_map();
    let args: Vec<String> = std::env::args().collect();
    let community_path = args.get(1).map(|s| s.as_str()).unwrap_or("clone-templates");
    let source_of_truth = args.get(2).map(|s| s.as_str()).unwrap_or("clone-templates/projectdiscovery/nuclei-templates");
    let output_path = args.get(3).map(|s| s.as_str()).unwrap_or("tmp");
    let poc_dir = args.get(4).map(|s| s.as_str()).unwrap_or("poc_all");

    let community = get_all_yaml_files(community_path);
    let nuclei = get_all_yaml_files(source_of_truth);

    let common_templates: HashSet<&String> = community.keys().filter(|k| nuclei.contains_key(*k)).collect();
    let mut file_hashes: HashMap<String, HashSet<String>> = HashMap::new();

    let poc_file_hashes = if Path::new(poc_dir).exists() {
        get_poc_file_hashes(poc_dir)?
    } else {
        HashSet::new()
    };

    for (template, community_file) in community.iter() {
        if common_templates.contains(template)
            && fs::metadata(community_file)?.len() == fs::metadata(&nuclei[template])?.len()
        {
            fs::remove_file(community_file)?;
            continue;
        }
        copy_file_if_unique(community_file, output_path, &cmap, &mut file_hashes, &poc_file_hashes)?;
    }

    // Clean up
    if Path::new("clone-templates").exists() {
        fs::remove_dir_all("clone-templates")?;
    }

    Ok(())
}

