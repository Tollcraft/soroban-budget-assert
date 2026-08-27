import json
import subprocess
import time
import os

def run_cmd(cmd, check=True, capture=True):
    print(f"Running: {cmd}")
    res = subprocess.run(cmd, shell=True, capture_output=capture, text=True)
    if check and res.returncode != 0:
        raise Exception(f"Command failed: {cmd}\nStdout: {res.stdout}\nStderr: {res.stderr}")
    return res

def get_open_prs():
    res = run_cmd("gh pr list --state open --json number,title,mergeable,statusCheckRollup,author,headRefName", capture=True)
    return json.loads(res.stdout)

def is_ci_passing(pr):
    rollup = pr.get("statusCheckRollup", [])
    if not rollup:
        return False
    # If any required check failed, return False
    for check in rollup:
        if check.get("conclusion") in ["FAILURE", "ACTION_REQUIRED", "TIMED_OUT"]:
            return False
        if check.get("status") != "COMPLETED":
            return False
    return True

def merge_pr(pr_num):
    print(f"Merging PR {pr_num}...")
    run_cmd(f"gh pr merge {pr_num} --squash --admin")
    print(f"Merged PR {pr_num} successfully.")

def comment_pr(pr_num, author, files):
    print(f"Commenting on PR {pr_num}...")
    # Get author login
    login = author.get("login")
    file_list = ", ".join(files[:5]) + ("..." if len(files) > 5 else "")
    msg = f"@{login} This PR has complex merge conflicts in `{file_list}` that could not be automatically resolved. Please rebase against `main` and resolve the conflicts when you have a chance."
    run_cmd(f"gh pr comment {pr_num} --body \"{msg}\"")

def process():
    # Make sure we are up to date
    run_cmd("git fetch origin main")
    run_cmd("git checkout main && git reset --hard origin/main")

    prs = get_open_prs()
    # Sort by number to process older PRs first
    prs.sort(key=lambda x: x["number"])

    for pr in prs:
        num = pr["number"]
        title = pr["title"]
        author = pr["author"]
        mergeable = pr["mergeable"]
        
        print(f"\n--- Processing PR #{num}: {title} ---")
        
        # Fresh state
        run_cmd("git checkout main && git reset --hard origin/main")
        run_cmd("git fetch origin main")
        
        if mergeable == "MERGEABLE" and is_ci_passing(pr):
            try:
                merge_pr(num)
                time.sleep(2)
                continue
            except Exception as e:
                print(f"Failed to merge cleanly: {e}")

        # If it's conflicting or CI failed, let's try to rebase
        print(f"Attempting rebase for PR #{num}...")
        try:
            run_cmd(f"gh pr checkout {num}")
            run_cmd("git fetch origin main")
            rebase_res = run_cmd("git rebase origin/main", check=False)
            
            if rebase_res.returncode != 0:
                # Conflict occurred
                status_res = run_cmd("git status --porcelain", check=True)
                conflicting_files = [line.split()[-1] for line in status_res.stdout.splitlines() if line.startswith("UU") or line.startswith("U ") or line.startswith(" U") or line.startswith("AA")]
                
                run_cmd("git rebase --abort")
                
                comment_pr(num, author, conflicting_files)
            else:
                # Rebase succeeded, let's run tests
                print("Rebase succeeded, running tests...")
                test_res = run_cmd("cargo test --workspace", check=False)
                if test_res.returncode == 0:
                    # Push and merge
                    print("Tests passed, pushing and merging...")
                    run_cmd("git push --force")
                    time.sleep(5)
                    merge_pr(num)
                else:
                    print("Tests failed after rebase.")
                    run_cmd(f"gh pr comment {num} --body \"@{author['login']} Tests fail after a clean rebase on `main`. Could you take a look?\"")
        except Exception as e:
            print(f"Error processing PR #{num}: {e}")
            run_cmd("git rebase --abort", check=False)

if __name__ == "__main__":
    process()
