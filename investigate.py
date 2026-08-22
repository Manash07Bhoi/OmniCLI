import os
import requests
import json
import sys

github_token = os.environ.get('GITHUB_TOKEN')
headers = {"Authorization": f"token {github_token}", "Accept": "application/vnd.github+json"}

# 1. Query remote tag
url_ref = "https://api.github.com/repos/Manash07Bhoi/OmniCLI/git/refs/tags/v0.1.1"
resp_ref = requests.get(url_ref, headers=headers)
if resp_ref.status_code == 200:
    ref_data = resp_ref.json()
    tag_sha = ref_data['object']['sha']
    # If it's an annotated tag, resolve it to the actual commit
    if ref_data['object']['type'] == 'tag':
        url_tag = f"https://api.github.com/repos/Manash07Bhoi/OmniCLI/git/tags/{tag_sha}"
        resp_tag = requests.get(url_tag, headers=headers)
        if resp_tag.status_code == 200:
            tag_sha = resp_tag.json()['object']['sha']
    print(f"Remote v0.1.1 Tag resolves to Commit SHA: {tag_sha}")
else:
    print(f"Failed to find v0.1.1 tag remotely: {resp_ref.status_code}")
    sys.exit(1)

# 2. Query failed runs for v0.1.1
url_runs = "https://api.github.com/repos/Manash07Bhoi/OmniCLI/actions/runs?branch=v0.1.1&status=failure"
resp_runs = requests.get(url_runs, headers=headers)
runs = resp_runs.json().get('workflow_runs', [])
if not runs:
    print("No failed runs found for v0.1.1")
    sys.exit(1)

latest_run = runs[0]
print(f"Latest Failed Run ID: {latest_run['id']}")
print(f"Run Head SHA: {latest_run['head_sha']}")
print(f"Run Ref: {latest_run['head_branch']}")

# 3. Retrieve release.yml at the run's Head SHA
import base64
url_file = f"https://api.github.com/repos/Manash07Bhoi/OmniCLI/contents/omnicli/.github/workflows/release.yml?ref={latest_run['head_sha']}"
resp_file = requests.get(url_file, headers=headers)
if resp_file.status_code == 200:
    content = base64.b64decode(resp_file.json()['content']).decode('utf-8')
    print("\n--- Snippet of release.yml at run's Head SHA ---")
    lines = content.split('\n')
    for i, line in enumerate(lines):
        if 'mv' in line or 'cp' in line or 'sha256sum' in line or 'download-artifact' in line:
            print(f"{i+1}: {line}")
else:
    print(f"Failed to fetch file at {latest_run['head_sha']}")

# 4. Also retrieve release.yml at current main
url_main = "https://api.github.com/repos/Manash07Bhoi/OmniCLI/contents/omnicli/.github/workflows/release.yml?ref=main"
resp_main = requests.get(url_main, headers=headers)
if resp_main.status_code == 200:
    content_main = base64.b64decode(resp_main.json()['content']).decode('utf-8')
    print("\n--- Snippet of release.yml at MAIN ---")
    lines = content_main.split('\n')
    for i, line in enumerate(lines):
        if 'mv' in line or 'cp' in line or 'sha256sum' in line or 'download-artifact' in line:
            print(f"{i+1}: {line}")
