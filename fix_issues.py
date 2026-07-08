import subprocess
import json
import re

for i in range(5, 18):
    print(f"Processing issue {i}...")
    result = subprocess.run(["gh", "issue", "view", str(i), "--json", "body"], capture_output=True, text=True)
    if result.returncode != 0:
        print(f"Failed to fetch issue {i}")
        continue
        
    try:
        body = json.loads(result.stdout)["body"]
    except Exception as e:
        print(f"Error parsing JSON for issue {i}: {e}")
        continue
    
    # Clean up any \r
    body = body.replace("\r\n", "\n")
    
    # We look for the previous footer
    pattern = r"---+\n\*\*Contact & Support\*\*.*?\nTelegram: (https://[^\n]+)\nDiscord: (https://[^\n]+)"
    
    def replacer(m):
        tg = m.group(1).strip()
        dc = m.group(2).strip()
        return f"---\n\n### **Contact & Support**\n- **Telegram:** [{tg}]({tg})\n- **Discord:** [{dc}]({dc})"
        
    new_body, count = re.subn(pattern, replacer, body, flags=re.DOTALL)
    
    if count > 0:
        # Edit the issue
        res = subprocess.run(["gh", "issue", "edit", str(i), "--body", new_body], capture_output=True, text=True)
        if res.returncode == 0:
            print(f"Updated issue {i}")
        else:
            print(f"Failed to update issue {i}: {res.stderr}")
    else:
        print(f"No match found for issue {i}. Here is the body snippet:")
        print(repr(body[-200:]))

