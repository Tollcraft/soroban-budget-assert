import json
import subprocess
try:
    result = subprocess.run(["gh", "pr", "list", "--state", "open", "--json", "number,title,mergeable,statusCheckRollup", "--limit", "100"], capture_output=True, text=True, check=True)
    prs = json.loads(result.stdout)
    
    # Filter for PRs that are MERGEABLE but maybe CI failed (since we can re-run CI or fix them easily)
    easiest_prs = []
    conflict_prs = []
    
    for pr in prs:
        if pr['number'] in (168, 177):
            continue
        if pr['mergeable'] == 'MERGEABLE':
            easiest_prs.append(pr)
        elif pr['mergeable'] == 'CONFLICTING':
            conflict_prs.append(pr)

    print("Easiest PRs (MERGEABLE):")
    for pr in easiest_prs:
        print(f"#{pr['number']}: {pr['title']} - {pr['mergeable']}")

    print("\nConflict PRs:")
    for pr in conflict_prs:
        print(f"#{pr['number']}: {pr['title']} - {pr['mergeable']}")

except Exception as e:
    print(f"Error: {e}")
