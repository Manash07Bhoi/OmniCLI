import os
import requests

github_token = os.environ.get('GITHUB_TOKEN')
headers = {"Authorization": f"token {github_token}", "Accept": "application/vnd.github+json"}

url = "https://api.github.com/repos/Manash07Bhoi/OmniCLI/actions/runs?branch=v0.1.1"
response = requests.get(url, headers=headers)
runs = response.json().get('workflow_runs', [])
for run in runs[:5]:
    print(f"Run ID: {run['id']}, Status: {run['status']}, Conclusion: {run['conclusion']}, Commit: {run['head_sha']}")
