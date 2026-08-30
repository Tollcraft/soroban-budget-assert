import json
import subprocess
import time
import sys

def run(cmd):
    print(f"Running: {cmd}")
    result = subprocess.run(cmd, shell=True, capture_output=True, text=True)
    return result

# Get PRs
out = run("gh pr list --state open --json number")
prs = [pr["number"] for pr in json.loads(out.stdout)]

for pr in prs:
    print(f"Attempting to merge PR {pr}...")
    res = run(f"gh pr merge {pr} --merge -d --admin")
    if res.returncode == 0:
        print(f"PR {pr} merged successfully.")
    else:
        print(f"Failed to merge PR {pr}. Output:")
        print(res.stderr)
        print("Stopping to allow manual conflict resolution.")
        sys.exit(1)
    time.sleep(2)

print("All PRs processed!")
