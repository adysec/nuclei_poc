import requests

def get_latest_release():
    url = "https://api.github.com/repos/projectdiscovery/nuclei/releases/latest"
    response = requests.get(url)
    if response.status_code == 200:
        return response.json()
    else:
        raise Exception("Failed to fetch latest release")

def download_file(url, dest):
    response = requests.get(url, stream=True)
    if response.status_code == 200:
        with open(dest, 'wb') as f:
            for chunk in response.iter_content(1024):
                f.write(chunk)
    else:
        raise Exception(f"Failed to download file: {url}")

def main():
    release = get_latest_release()
    assets = release['assets']
    
    download_url = None
    for asset in assets:
        if 'linux' in asset['name'] and 'amd64' in asset['name'] and asset['name'].endswith('.zip'):
            download_url = asset['browser_download_url']
            break

    if not download_url:
        raise Exception("Linux amd64 ZIP asset not found in the latest release")

    print(f"Downloading from {download_url}")

    dest_file = 'nuclei.zip'
    download_file(download_url, dest_file)

    print("Downloaded Nuclei ZIP file")

if __name__ == "__main__":
    main()
