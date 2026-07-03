#![cfg(test)]

use soroban_sdk::Env;
use example_contract::{ExpensiveContract, ExpensiveContractClient};
use budget_macros::budget_cpu_lt;

#[test]
fn test_budget_raw_rust() {
    let env = Env::default();
    let contract_id = env.register(ExpensiveContract, ());
    let client = ExpensiveContractClient::new(&env, &contract_id);
    
    // Enable cost estimation
    env.cost_estimate().budget().reset_unlimited();

    client.do_expensive_work(&10_000);

    let budget = env.cost_estimate().budget();
    println!("=== RAW RUST LOCAL ===");
    println!("CPU instructions: {}", budget.cpu_instruction_cost());
    println!("Memory bytes: {}", budget.memory_bytes_cost());
}

#[test]
fn test_budget_wasm() {
    let env = Env::default();
    
    // Path to the compiled wasm
    let wasm_path = "../target/wasm32-unknown-unknown/release/example_contract.wasm";
    let wasm = std::fs::read(wasm_path).expect("WASM file not found, did you run cargo build?");
    #[allow(deprecated)]
    let contract_id = env.register_contract_wasm(None, wasm.as_slice());
    let client = ExpensiveContractClient::new(&env, &contract_id);
    
    env.cost_estimate().budget().reset_unlimited();

    client.do_expensive_work(&10_000);

    let budget = env.cost_estimate().budget();
    println!("=== WASM LOCAL ===");
    println!("CPU instructions: {}", budget.cpu_instruction_cost());
    println!("Memory bytes: {}", budget.memory_bytes_cost());
}

#[test]
#[budget_cpu_lt(850000)] // Using the discovered margin (WASM local 767049, actual testnet ~832006)
fn test_budget_macro_gated() {
    let env = Env::default();
    
    // Path to the compiled wasm
    let wasm_path = "../target/wasm32-unknown-unknown/release/example_contract.wasm";
    let wasm = std::fs::read(wasm_path).expect("WASM file not found, did you run cargo build?");
    #[allow(deprecated)]
    let contract_id = env.register_contract_wasm(None, wasm.as_slice());
    let client = ExpensiveContractClient::new(&env, &contract_id);
    
    env.cost_estimate().budget().reset_unlimited();

    client.do_expensive_work(&10_000); // Should pass as it's < 850000 CPU limit
}

#[test]
#[should_panic(expected = "local estimate, underestimates real network cost")]
#[budget_cpu_lt(600000)] // Deliberate regression
fn test_budget_macro_deliberate_regression() {
    let env = Env::default();
    
    // Path to the compiled wasm
    let wasm_path = "../target/wasm32-unknown-unknown/release/example_contract.wasm";
    let wasm = std::fs::read(wasm_path).expect("WASM file not found, did you run cargo build?");
    #[allow(deprecated)]
    let contract_id = env.register_contract_wasm(None, wasm.as_slice());
    let client = ExpensiveContractClient::new(&env, &contract_id);
    
    env.cost_estimate().budget().reset_unlimited();

    client.do_expensive_work(&10_000); 
}
