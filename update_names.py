import os

files_to_update = [
    ".github/workflows/budget.yml",
    "Cargo.toml",
    "docs/src/contributing.md",
    "docs/src/developer_guide.md",
    "docs/src/reference.md",
    "amm-pool-contract/Cargo.toml",
    "amm-pool-contract/tests/budget_test.rs",
    "record_demo.sh",
    "submission.md",
    "scripts/test_network.js"
]

for file_path in files_to_update:
    if not os.path.exists(file_path):
        continue
    with open(file_path, "r") as f:
        content = f.read()
    
    # Replace the package name
    content = content.replace("example-contract", "amm-pool-contract")
    # Replace the rust module name / wasm name
    content = content.replace("example_contract", "amm_pool_contract")
    
    with open(file_path, "w") as f:
        f.write(content)

# Now update record_demo.sh to speed it up by 30%
with open("record_demo.sh", "r") as f:
    demo_content = f.read()

demo_content = demo_content.replace("sleep 0.05", "sleep 0.035")
demo_content = demo_content.replace("sleep 0.5", "sleep 0.35")
demo_content = demo_content.replace("sleep 1", "sleep 0.7")
demo_content = demo_content.replace("sleep 2", "sleep 1.4")

with open("record_demo.sh", "w") as f:
    f.write(demo_content)

print("Updates completed successfully.")
