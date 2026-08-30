import glob

for fpath in glob.glob(".github/workflows/*.yml"):
    with open(fpath, "r") as f:
        text = f.read()
    
    # Whenever amm-pool-contract is built, also build host-function-contract
    if "cargo build -p amm-pool-contract --release --target wasm32v1-none" in text:
        text = text.replace("cargo build -p amm-pool-contract --release --target wasm32v1-none",
                            "cargo build -p amm-pool-contract --release --target wasm32v1-none\n        cargo build -p host-function-contract --release --target wasm32v1-none")
    
    # Whenever amm-pool-contract is excluded from llvm-cov, also exclude host-function-contract
    if "--exclude amm-pool-contract \\" in text:
        text = text.replace("--exclude amm-pool-contract \\",
                            "--exclude amm-pool-contract \\\n            --exclude host-function-contract \\")
                            
    with open(fpath, "w") as f:
        f.write(text)
