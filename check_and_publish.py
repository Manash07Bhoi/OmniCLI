import urllib.request
import json
import subprocess
import time
import sys

# Define our remaining DAG in order
remaining_crates = [
    "omnicli-convert",
    "omnicli-dev",
    "omnicli-search",
    "omnicli-workspace",
    "omnicli-cli"
]

def check_crate_published(name):
    url = f"https://crates.io/api/v1/crates/{name}"
    req = urllib.request.Request(url, headers={'User-Agent': 'OmniCLI-Release-Bot/1.0'})
    try:
        urllib.request.urlopen(req)
        return True
    except urllib.error.HTTPError as e:
        if e.code == 404:
            return False
        else:
            print(f"crates.io check failed with HTTP {e.code}")
            return False
    except Exception as e:
        print(f"crates.io check failed: {e}")
        return False

def publish_crate(name):
    # Depending on crate folder vs cargo package name mapping
    package_name = name
    if name == "omnicli-cli":
        package_name = "omnicli-app"

    print(f"Publishing {name} (package {package_name})...")
    result = subprocess.run(["cargo", "publish", "-p", package_name], cwd="omnicli", capture_output=True, text=True)
    if result.returncode == 0:
        print(f"Success: {name}")
        return True
    else:
        print(f"Failed to publish {name}")
        print(result.stdout)
        print(result.stderr)
        return False

for crate in remaining_crates:
    pkg_name = "omnicli-app" if crate == "omnicli-cli" else crate
    print(f"Checking if {pkg_name} is already published...")
    if check_crate_published(pkg_name):
        print(f"{pkg_name} is already published. Skipping.")
        continue

    # Try to publish
    success = publish_crate(crate)
    if not success:
        print("Stopping due to publication failure (likely HTTP 429).")
        sys.exit(1)

    # Wait for propagation before publishing dependents
    print("Waiting 15 seconds for crates.io index propagation...")
    time.sleep(15)

print("All remaining crates published successfully.")
