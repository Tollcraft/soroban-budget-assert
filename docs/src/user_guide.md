# End-User Guide

This guide is for Soroban developers who want to integrate budget assertions into their existing smart contracts.

### Step 1: Install the CLI
Install the report generator from the project root:
```bash
cargo install --path cargo-budget-report
```

### Step 2: Configure Workspace
Create a `budget.toml` file in the root of your workspace:
```toml
network = "testnet"
source = "alice"

[functions.my_function]
args = ["--arg1", "value"]
```

### Step 3: Run the Report
Execute the report generator to simulate all functions in your workspace against the testnet:
```bash
cargo budget-report
```
Review the table. Identify functions that approach the network limit.
