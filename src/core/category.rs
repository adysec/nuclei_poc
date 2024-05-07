//! File-to-category keyword mapping used by the classification pipeline (step 3).
//!
//! Categories are derived from filename keywords. When no keyword matches a file
//! it is placed in `other/`.

use std::collections::{HashMap, HashSet};

/// Returns the canonical category → keyword list mapping.
///
/// Each keyword is matched case-insensitively against the **filename** (not the
/// full path). The first match wins; multiple matches assign the file to every
/// matching category.
pub fn category_map() -> HashMap<&'static str, Vec<&'static str>> {
    let mut m: HashMap<&str, Vec<&str>> = HashMap::new();
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

/// Classify a filename into one or more categories.
///
/// Returns `vec!["other"]` when no keyword matches.
pub fn classify_file(file_name: &str, cmap: &HashMap<&str, Vec<&str>>) -> Vec<String> {
    let name = file_name.to_lowercase();
    let mut cats: Vec<String> = Vec::new();
    for (cat, keywords) in cmap.iter() {
        if keywords.iter().any(|k| name.contains(k)) {
            cats.push((*cat).to_string());
        }
    }
    if cats.is_empty() {
        cats.push("other".to_string());
    }
    cats
}

/// Enhanced classification using filename + tags + request paths.
///
/// Priority: tags > filename keywords > path patterns > "other"
/// `tags` are lowercased nuclei info.tags strings;
/// `paths` are lowercased URL paths from requests.
pub fn classify_enhanced(
    file_name: &str,
    tags: &[String],
    paths: &[String],
    cmap: &HashMap<&str, Vec<&str>>,
) -> Vec<String> {
    let mut cats: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    // 1. Tags-based classification (highest priority)
    let tags_text = tags.join(" ").to_lowercase();
    for (cat, keywords) in cmap.iter() {
        if keywords.iter().any(|k| tags_text.contains(k)) {
            if seen.insert((*cat).to_string()) {
                cats.push((*cat).to_string());
            }
        }
    }

    // 2. Filename-based (if not already found via tags)
    let name = file_name.to_lowercase();
    for (cat, keywords) in cmap.iter() {
        if keywords.iter().any(|k| name.contains(k)) {
            if seen.insert((*cat).to_string()) {
                cats.push((*cat).to_string());
            }
        }
    }

    // 3. Path-based fallback
    let paths_text = paths.join(" ").to_lowercase();
    let path_rules: &[(&str, &[&str])] = &[
        ("wordpress", &["wp-admin", "wp-content", "wp-json", "wp-login"]),
        ("drupal", &["drupal", "/user/login"]),
        ("joomla", &["joomla", "com_content"]),
        ("jenkins", &["jenkins", "/job/", "/computer/"]),
        ("git", &["/.git/", "gitlab", "github"]),
        ("config", &["/config", "/.env", "/settings"]),
        ("php", &[".php", "phpmyadmin", "phpparam"]),
        ("api", &["/api/", "/graphql", "/rest/", "/v1/", "/v2/"]),
        ("sql", &["sql", "database", "/db/"]),
        ("auth", &["/login", "/signin", "/oauth", "/sso", "/auth"]),
        ("upload", &["/upload", "/file/"]),
        ("backup", &["/backup", ".bak", ".backup"]),
        ("debug", &["/debug", "/test", "/dev/"]),
        ("docker", &["/docker", "/containers"]),
        ("aws", &[".amazonaws.com", "s3.", "/aws/"]),
    ];

    for (cat, patterns) in path_rules {
        if patterns.iter().any(|p| paths_text.contains(p)) {
            if seen.insert((*cat).to_string()) {
                cats.push((*cat).to_string());
            }
        }
    }

    if cats.is_empty() {
        cats.push("other".to_string());
    }
    cats
}
