#!/bin/bash
# record_demo.sh - A script to automate typing and executing commands for Asciinema

# Function to simulate typing
type_command() {
    local cmd="$1"
    echo -ne "$ "
    for (( i=0; i<${#cmd}; i++ )); do
        echo -ne "${cmd:$i:1}"
        sleep 0.035
    done
    sleep 0.35
    echo ""
    eval "$cmd"
    echo ""
    sleep 0.7
}

clear
echo -e "\033[1;36m=== Soroban Budget Assert Demo ===\033[0m\n"
sleep 0.7

# Step 1: Build
type_command "cargo build -p amm-pool-contract --release --target wasm32-unknown-unknown"

# Step 2: Show the macro catching a regression locally
echo -e "\033[1;33m# Let's run a test where our function exceeds our hardcoded #[budget_cpu_lt(600000)] limit\033[0m"
type_command "cargo test -p amm-pool-contract --test budget_test test_budget_macro_deliberate_regression"

# Step 3: Run the CLI report
echo -e "\033[1;33m# Now let's generate a network-verified workspace report!\033[0m"
type_command "cargo run --bin cargo-budget-report -- budget-report"

echo -e "\033[1;32mDemo Complete!\033[0m"
sleep 1.4
