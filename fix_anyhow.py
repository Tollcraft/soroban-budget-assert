with open("cargo-budget-report/src/main.rs", "r") as f:
    text = f.read()

text = text.replace(
    'anyhow::bail!("budget.toml validation failed:\\n{report}");',
    'return Err(Error::Message(format!("budget.toml validation failed:\\n{report}")));'
)

with open("cargo-budget-report/src/main.rs", "w") as f:
    f.write(text)
