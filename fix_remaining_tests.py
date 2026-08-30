import re

for fname in ["cargo-budget-report/src/edge_case_tests.rs", "cargo-budget-report/src/additional_edge_tests.rs"]:
    with open(fname, "r") as f:
        text = f.read()
    
    # Catch any build_invoke_args(...) missing None
    text = re.sub(
        r'(build_invoke_args\(\s*(?:"[^"]*",\s*){4}\&\[.*?\]\s*)\)',
        r'\1, None)',
        text,
        flags=re.DOTALL
    )
    with open(fname, "w") as f:
        f.write(text)

with open("cargo-budget-report/src/main.rs", "r") as f:
    text = f.read()

text = text.replace(
    "no_deploy_cache: false,\n",
    "no_deploy_cache: false,\n            source_secret: None,\n"
)

with open("cargo-budget-report/src/main.rs", "w") as f:
    f.write(text)
