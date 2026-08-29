with open("README.md", "r") as f:
    text = f.read()

text = text.replace("<<<<<<< HEAD\n", "").replace("=======\n>>>>>>> origin/main\n", "")

with open("README.md", "w") as f:
    f.write(text)

with open("cargo-budget-report/Cargo.toml", "r") as f:
    text = f.read()

import re
text = re.sub(
    r"<<<<<<< HEAD\nclap = .*?\nsha2 = .*?\n=======\nclap = .*?\nclap_mangen = .*?\n>>>>>>> origin/main",
    r'clap = { version = "4.4", features = ["derive", "env"] }\nsha2 = "0.10"\nclap_mangen = "0.2"',
    text,
    flags=re.DOTALL
)

with open("cargo-budget-report/Cargo.toml", "w") as f:
    f.write(text)
