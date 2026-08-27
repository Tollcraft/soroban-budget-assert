with open(".github/workflows/budget.yml", "r") as f:
    text = f.read()

text = text.replace(
    "run: cargo build -p amm-pool-contract --release --target wasm32v1-none",
    "run: |\n          cargo build -p amm-pool-contract --release --target wasm32v1-none\n          cargo build -p host-function-contract --release --target wasm32v1-none"
)

with open(".github/workflows/budget.yml", "w") as f:
    f.write(text)

with open(".github/workflows/coverage.yml", "r") as f:
    text = f.read()

text = text.replace(
    "--exclude amm-pool-contract \\",
    "--exclude amm-pool-contract \\\n            --exclude host-function-contract \\"
)

with open(".github/workflows/coverage.yml", "w") as f:
    f.write(text)
