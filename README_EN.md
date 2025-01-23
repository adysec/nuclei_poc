# Nuclei POCs

<a href="https://github.com/adysec/nuclei_poc/stargazers"><img alt="GitHub Repo stars" src="https://img.shields.io/github/stars/adysec/nuclei_poc?color=yellow&logo=riseup&logoColor=yellow&style=flat-square"></a>
<a href="https://github.com/adysec/nuclei_poc/network/members"><img alt="GitHub forks" src="https://img.shields.io/github/forks/adysec/nuclei_poc?color=orange&style=flat-square"></a>
<a href="https://github.com/adysec/nuclei_poc/issues"><img alt="GitHub issues" src="https://img.shields.io/github/issues/adysec/nuclei_poc?color=red&style=flat-square"></a>

Daily updated Nuclei Proof-of-Concept (POC) repository.

[English](https://github.com/adysec/nuclei_poc/blob/main/README_EN.md) | [中文](https://github.com/adysec/nuclei_poc/blob/main/README.md)

This project is a Python script for batch cloning GitHub repositories, extracting Nuclei POCs, and organizing the POCs into categorized folders. The script runs automatically every day using GitHub Actions.

The POC format validation code has been updated and optimized. When the tmp/ directory does not exist, all POC format checks will be completed.

After format validation, only 117k usable POCs remain, with 111k unique POCs after deduplication. The previously calculated count of 140k+ was incorrect and has been corrected.

## How to Use

Clone the repository and navigate to the project directory:

```bash
git clone https://github.com/adysec/nuclei_poc
cd nuclei_poc
```

Use Nuclei to scan websites with POCs:

```bash
./nuclei -t poc/ -u http://example.com
# Scan specific categories of POCs
./nuclei -t poc/web/ -u http://example.com
./nuclei -t poc/wordpress/ -u http://example.com
```

### Configuration

Configure the monitored GitHub repositories in the `repo.csv` file.

### GitHub Actions

Set up Actions in the GitHub repository to automatically run the script daily.

> Make sure to set Workflow permissions to Read and write.

## File Structure


- `1-clone_repos.py`: Clone monitored GitHub repositories in bulk.
- `2-delete_duplicated.py`: Remove duplicate POC scripts.
- `3-move_file.py`: Archive POC scripts into the tmp directory.
- `4-download_nuclei.py`: Download Nuclei to validate POC scripts.
- `5-check_poc.sh`: Validate POC scripts and move them to the poc directory.
- `6-get_count.py`: Get the count of archived POC scripts.
- `7-get_pocname.py`: Read and write the list of POCs into poc.txt.
- `check_poc.sh`: Validate POC scripts and package them into a poc.zip file.
- `repo.csv`: List of GitHub repositories containing Nuclei POCs.
- `poc.txt`: List of archived POC scripts.
- `poc/`: Categorized Nuclei POC files.
- `clone-templates/`: Temporary folder for cloned GitHub projects.
- `tmp/`: Temporary folder for deduplicated and categorized POC scripts.

## Acknowledgments

This project has received support and contributions from various individuals and projects. Special thanks to the following:

### Projects

Thanks to [ProjectDiscovery](https://github.com/projectdiscovery/nuclei) for providing the Nuclei tool and support to the open-source community.

### Individuals

Special thanks to [TajangSec](https://github.com/TajangSec) for optimizing and improving parts of the code.
