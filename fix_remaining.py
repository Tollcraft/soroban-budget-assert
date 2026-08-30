import re

# Fix build_invoke_args multiline
for fname in ["cargo-budget-report/src/edge_case_tests.rs", "cargo-budget-report/src/additional_edge_tests.rs"]:
    with open(fname, "r") as f:
        text = f.read()
    
    # Catch any build_invoke_args(...) missing None
    # Just do a lazy replace: find build_invoke_args(...), check if it has 5 args
    text = re.sub(
        r'(build_invoke_args\(\s*(?:"[^"]*",\s*){4}\&\[.*?\](?:.into\(\))?\s*)\)',
        r'\1, None)',
        text
    )
    # the multiline ones:
    text = re.sub(
        r'(build_invoke_args\(\s*"[^"]*",\s*"[^"]*",\s*"[^"]*",\s*"[^"]*",\s*\&\[[^\]]*\]\s*)\)',
        r'\1, None)',
        text
    )
    with open(fname, "w") as f:
        f.write(text)

# Fix main.rs
with open("cargo-budget-report/src/main.rs", "r") as f:
    text = f.read()

text = text.replace("offline: false,\n", "")
text = text.replace("csv::Writer::from_writer(vec![], None)", "csv::Writer::from_writer(vec![])")

with open("cargo-budget-report/src/main.rs", "w") as f:
    f.write(text)
