import os
import re

test_files = [
    "cargo-budget-report/src/edge_case_tests.rs",
    "cargo-budget-report/src/boundary_tests.rs",
    "cargo-budget-report/src/additional_edge_tests.rs",
    "cargo-budget-report/src/main.rs",
]

# For build_invoke_args, it takes:
# build_invoke_args(contract_id, source_account, network, function, args, rpc_override)
# In edge_case_tests.rs, boundary_tests.rs, additional_edge_tests.rs, it has 5 args.

for file in test_files:
    if not os.path.exists(file):
        continue
    with open(file, "r") as f:
        text = f.read()

    # Simple regex for build_invoke_args(..., ..., ..., ..., ...)
    # Wait, some are multi-line. We'll use a regex that matches `build_invoke_args(` up to the closing `)`
    # This might be tricky. Let's just do a naive substitution for the single line ones:
    text = re.sub(r'build_invoke_args\(([^,]+, [^,]+, [^,]+, [^,]+, [^,]+(?:\.into\(\))?(?:, )?\[.*?\](?:, )?(?:\/\*.*?\*\/)?)\)', r'build_invoke_args(\1, None)', text, flags=re.DOTALL)
    
    # Actually, an easier way is to just grep and sed or use Python ast? 
    # Or just redefine build_invoke_args in those tests? No, they import it.
    
    with open(file, "w") as f:
        f.write(text)
