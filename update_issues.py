import subprocess
import json
import sys

def run_cmd(cmd):
    result = subprocess.run(cmd, capture_output=True, text=True, shell=True)
    if result.returncode != 0:
        print(f"Command failed: {cmd}\n{result.stderr}")
        return None
    return result.stdout.strip()

def main():
    issues_out = run_cmd("gh issue list --json number -L 100")
    if not issues_out:
        return
    issues = json.loads(issues_out)
    
    append_text = """
---
### 📋 Before you start
Please read our [Code Quality Standards](https://github.com/Tollcraft/soroban-budget-assert/blob/main/CONTRIBUTING.md#code-quality-standards) in `CONTRIBUTING.md`. Before submitting a PR, ensure you run:
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
"""

    for issue in issues:
        num = issue["number"]
        print(f"Processing issue {num}...")
        body_out = run_cmd(f"gh issue view {num} --json body")
        if not body_out:
            print(f"Failed to fetch body for {num}")
            continue
        
        body_data = json.loads(body_out)
        body = body_data.get("body", "")
        
        if not body.strip():
            print(f"Warning: Issue {num} has empty body, skipping to be safe.")
            continue
            
        if "📋 Before you start" in body:
            print(f"Issue {num} already updated.")
            continue
            
        new_body = body.rstrip() + "\n" + append_text
        
        # Write to temp file to update
        with open(f"/tmp/issue_{num}.md", "w") as f:
            f.write(new_body)
            
        res = run_cmd(f"gh issue edit {num} --body-file /tmp/issue_{num}.md")
        if res is not None:
            print(f"Successfully updated issue {num}")
        else:
            print(f"Failed to update issue {num}")

if __name__ == "__main__":
    main()
