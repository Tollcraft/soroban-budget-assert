with open("cargo-budget-report/src/main.rs", "r") as f:
    text = f.read()

import re

text = re.sub(
    r"<<<<<<< HEAD\n(.*?)=======\n(.*?)>>>>>>> origin/main\n",
    r"\1\n\2",
    text,
    flags=re.DOTALL
)

with open("cargo-budget-report/src/main.rs", "w") as f:
    f.write(text)
