import re

for filename in ["CHANGELOG.md", "docs/src/ci_cd_integration.md"]:
    with open(filename, "r") as f:
        text = f.read()

    # Simple regex to merge both HEAD and origin/main blocks
    text = re.sub(
        r"<<<<<<< HEAD\n(.*?)=======\n(.*?)>>>>>>> origin/main\n",
        r"\1\2",
        text,
        flags=re.DOTALL
    )

    with open(filename, "w") as f:
        f.write(text)
