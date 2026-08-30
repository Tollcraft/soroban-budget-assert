with open("cargo-budget-report/src/main.rs", "r") as f:
    text = f.read()

import re

# Block 1: Keep HEAD
text = re.sub(
    r"<<<<<<< HEAD\n(.*?)\n=======\n(?:.*?)\n>>>>>>> origin/main\n",
    r"\1\n",
    text,
    count=1,
    flags=re.DOTALL
)

# Block 2: Keep origin/main
text = re.sub(
    r"<<<<<<< HEAD\n(?:.*?)\n=======\n(.*?)\n>>>>>>> origin/main\n",
    r"\1\n",
    text,
    count=1,
    flags=re.DOTALL
)

with open("cargo-budget-report/src/main.rs", "w") as f:
    f.write(text)

with open("cargo-budget-report/tests/integration.rs", "r") as f:
    text_test = f.read()

# Block 3: Keep origin/main
text_test = re.sub(
    r"<<<<<<< HEAD\n(?:.*?)\n=======\n(.*?)\n>>>>>>> origin/main\n",
    r"\1\n",
    text_test,
    count=1,
    flags=re.DOTALL
)

with open("cargo-budget-report/tests/integration.rs", "w") as f:
    f.write(text_test)
