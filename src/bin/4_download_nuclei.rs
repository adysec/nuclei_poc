use reqwest::Client;
use clap::Parser;
use tokio::fs as tokio_fs;
use tokio::time::{sleep, Duration};
use tracing::warn;
use serde::Deserialize;
use std::fs::{self, File};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::io;
use zip::ZipArchive;
use std::path::PathBuf;
// (no duplicated imports)

#[derive(Debug, Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug, Deserialize)]
struct Release {
    assets: Vec<Asset>,
}

async fn get_latest_release() -> anyhow::Result<Release> {
    let url = "https://api.github.com/repos/projectdiscovery/nuclei/releases/latest";
    let client = Client::builder().user_agent("nuclei_poc/1.0").build()?;
    let resp = client.get(url).send().await?;
    if resp.status().is_success() {
        let release = resp.json::<Release>().await?;
        Ok(release)
    } else {
        Err(anyhow::anyhow!("Failed to fetch latest release: {}", resp.status()))
    }
}

// Extract only nuclei binary from zip file into extract_dir and return extracted path
fn extract_nuclei_from_zip(zip_path: &str, extract_dir: &str) -> anyhow::Result<Option<PathBuf>> {
    let file = File::open(zip_path)?;
    let mut archive = ZipArchive::new(file)?;
    let mut found: Option<PathBuf> = None;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        if entry.is_dir() { continue; }
        if let Some(enclosed) = entry.enclosed_name() {
            if let Some(fname) = enclosed.file_name().and_then(|s| s.to_str()) {
                let fname_lc = fname.to_lowercase();
                // Match files whose name starts with nuclei and are not docs (md) files
                if fname_lc.starts_with("nuclei") && !fname_lc.ends_with(".md") && !fname_lc.ends_with(".txt") && !fname_lc.ends_with(".yml") && !fname_lc.ends_with(".yaml") {
                    // Extract only binary file (flatten path — put directly in extract_dir)
                    let outpath = Path::new(extract_dir).join(fname);
                    if let Some(pdir) = outpath.parent() { fs::create_dir_all(pdir)?; }
                    let mut outfile = File::create(&outpath)?;
                    io::copy(&mut entry, &mut outfile)?;
                    found = Some(outpath);
                    break; // extract only first matching file
                }
            }
        }
    }
    Ok(found)
}

async fn download_file(url: &str, dest: &str, retries: usize) -> anyhow::Result<()> {
    let client = Client::builder().user_agent("nuclei_poc/1.0").build()?;
    let mut attempt: usize = 0;
    loop {
        attempt += 1;
        let resp = client.get(url).send().await;
        match resp {
            Ok(r) => {
                if !r.status().is_success() {
                    if attempt > retries { return Err(anyhow::anyhow!("Failed to download file: {}", url)); }
                    warn!("http error {}, retrying (attempt {}/{})", r.status(), attempt, retries);
                    sleep(Duration::from_secs(1 << attempt)).await;
                    continue;
                }
                let body = r.bytes().await?;
                tokio_fs::write(dest, &body).await?;
                return Ok(());
            }
            Err(e) => {
                if attempt > retries { return Err(anyhow::anyhow!("Download failed: {}", e)); }
                warn!("download error: {} -> retrying (attempt {}/{})", e, attempt, retries);
                sleep(Duration::from_secs(1 << attempt)).await;
                continue;
            }
        }
    }
}

#[derive(Parser)]
struct Args {
    #[clap(default_value = "nuclei.zip")]
    dest_file: String,
    #[clap(short, long, default_value_t = 3)]
    retries: usize,
    /// set to "extract" to extract the downloaded zip
    #[clap(short, long, default_value = "extract")]
    do_extract: String,
    #[clap(long, default_value = ".")]
    extract_dir: String,
    #[clap(long, default_value_t = 0)]
    jobs: usize,
}
fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let jobs_final = if args.jobs == 0 { let n = num_cpus::get(); if n==0 {1} else {n} } else { args.jobs };
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(jobs_final).enable_all().build()?;
    return rt.block_on(async_main(args));
}

async fn async_main(args: Args) -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let dest_file = args.dest_file.as_str();
    let retries = args.retries;
    let do_extract = args.do_extract.as_str();
    let extract_dir = args.extract_dir.as_str();
    let release = get_latest_release().await?;
    let mut download_url: Option<String> = None;
    for asset in release.assets.iter() {
        let name = asset.name.to_lowercase();
        if name.contains("linux") && name.contains("amd64") && name.ends_with(".zip") {
            download_url = Some(asset.browser_download_url.clone());
            break;
        }
    }
    let download_url = download_url.ok_or_else(|| anyhow::anyhow!("Linux amd64 ZIP asset not found"))?;
    println!("Downloading from {}", download_url);
    if Path::new(dest_file).exists() {
        println!("Destination file {} already exists, skipping download.", dest_file);
    } else {
        download_file(&download_url, dest_file, retries).await?;
    }
    if do_extract == "extract" {
        println!("Extracting {}", dest_file);
        let dest_file_owned = dest_file.to_string();
        let extract_dir_owned = extract_dir.to_string();
        let res = tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            if let Some(nuclei_path) = extract_nuclei_from_zip(&dest_file_owned, &extract_dir_owned)? {
                let mut perms = fs::metadata(&nuclei_path)?.permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&nuclei_path, perms)?;
                let dest = Path::new("./nuclei");
                if nuclei_path != dest {
                    if dest.exists() { fs::remove_file(dest).ok(); }
                    fs::rename(&nuclei_path, dest)?;
                }
                println!("Nuclei binary extracted and installed to ./nuclei");
            } else {
                println!("Warning: nuclei binary not found inside the extracted zip; you may need to locate it manually");
            }
            fs::remove_file(&dest_file_owned).ok();
            println!("Removed {}", dest_file_owned);
            Ok(())
        }).await?;
        res?;
    }
    println!("Downloaded Nuclei ZIP file");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    // use fully qualified FileOptions in this test to avoid type inference issues
    use std::io::Write;

    #[test]
    fn test_extract_nuclei_from_zip_only_binary() {
        let td = tempdir().unwrap();
        let zip_path = td.path().join("test.zip");
        // Create zip with README and nuclei files
        {
            let zipfile = File::create(&zip_path).unwrap();
            let mut zip = zip::ZipWriter::new(zipfile);
            let options_md: zip::write::FileOptions<'_, zip::write::ExtendedFileOptions> = zip::write::FileOptions::default().unix_permissions(0o644);
            let options_bin: zip::write::FileOptions<'_, zip::write::ExtendedFileOptions> = zip::write::FileOptions::default().unix_permissions(0o755);
            zip.start_file("README.md", options_md).unwrap();
            zip.write_all(b"this is a readme").unwrap();
            zip.start_file("nuclei", options_bin).unwrap();
            zip.write_all(b"binary content").unwrap();
            zip.finish().unwrap();
        }
        let extract_dir = td.path().join("out");
        fs::create_dir_all(&extract_dir).unwrap();
        let zip_path_str = zip_path.to_str().unwrap();
        let extract_dir_str = extract_dir.to_str().unwrap();
        // call
        let found = extract_nuclei_from_zip(zip_path_str, extract_dir_str).unwrap();
        assert!(found.is_some());
        let found_path = found.unwrap();
        let contents = fs::read_to_string(found_path.clone()).unwrap();
        assert_eq!(contents, "binary content");
        // ensure README not extracted
        assert!(!extract_dir.join("README.md").exists());
    }
}

