import os
import requests
import zipfile
import io

github_token = os.environ.get('GITHUB_TOKEN')
headers = {"Authorization": f"token {github_token}", "Accept": "application/vnd.github+json"}

url = "https://api.github.com/repos/Manash07Bhoi/OmniCLI/actions/runs/32552463610/logs"
response = requests.get(url, headers=headers, stream=True)

if response.status_code == 200:
    with zipfile.ZipFile(io.BytesIO(response.content)) as z:
        for filename in z.namelist():
            if "Create GitHub Release" in filename:
                with z.open(filename) as f:
                    print(f"--- {filename} ---")
                    print(f.read().decode('utf-8'))
