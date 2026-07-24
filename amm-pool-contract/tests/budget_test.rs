#![cfg(test)]

use amm_pool_contract::{ExpensiveContract, ExpensiveContractClient};
use budget_macros::{budget_cpu_lt, budget_mem_lt};
use soroban_sdk::Env;

fn setup_wasm_client(env: &Env) -> ExpensiveContractClient {
    let wasm_path = "../target/wasm32-unknown-unknown/release/amm_pool_contract.wasm";
    let wasm = std::fs::read(wasm_path).expect("WASM file not found, did you run cargo build?");
    #[allow(deprecated)]
    let contract_id = env.register_contract_wasm(None, wasm.as_slice());
    let client = ExpensiveContractClient::new(env, &contract_id);
    env.cost_estimate().budget().reset_unlimited();
    client
}

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
    let client = setup_wasm_client(&env);

    client.do_expensive_work(&10_000);

    let budget = env.cost_estimate().budget();
    println!("=== WASM LOCAL ===");
    println!("CPU instructions: {}", budget.cpu_instruction_cost());
    println!("Memory bytes: {}", budget.memory_bytes_cost());
}

#[test]
#[budget_cpu_lt(950000)] // Re-measured with the Soroban release profile applied: WASM local 901816, actual testnet ~756678
fn test_budget_macro_gated() {
    let env = Env::default();
    let client = setup_wasm_client(&env);

    client.do_expensive_work(&10_000); // Should pass as it's < 850000 CPU limit
}

#[test]
#[should_panic(
    expected = "local estimate, real network cost may differ significantly in either direction"
)]
#[budget_cpu_lt(600000)] // Deliberate regression
fn test_budget_macro_deliberate_regression() {
    let env = Env::default();
    let client = setup_wasm_client(&env);

    client.do_expensive_work(&10_000);
}

#[test]
#[should_panic(
    expected = "local estimate, real network cost may differ significantly in either direction"
)]
#[budget_mem_lt(1)] // Deliberate regression: any real memory cost exceeds an impossible 1-byte limit
fn test_budget_macro_mem_deliberate_regression() {
    let env = Env::default();

    // Path to the compiled wasm
    let wasm_path = "../target/wasm32-unknown-unknown/release/amm_pool_contract.wasm";
    let wasm = std::fs::read(wasm_path).expect("WASM file not found, did you run cargo build?");
    #[allow(deprecated)]
    let contract_id = env.register_contract_wasm(None, wasm.as_slice());
    let client = ExpensiveContractClient::new(&env, &contract_id);

    env.cost_estimate().budget().reset_unlimited();

    client.do_expensive_work(&10_000);
}

#[test]
#[budget_cpu_lt(env = "TEST_MAX_CPU")]
fn test_budget_macro_dynamic_env() {
    let budget_env_resolve = |var: &str| -> Option<String> {
        if var == "TEST_MAX_CPU" { Some("950000".to_string()) } else { None }
    };
    let env = Env::default();
    let client = setup_wasm_client(&env);

    client.do_expensive_work(&10_000);
}

#[test]
#[budget_cpu_lt(env = "TEST_MAX_CPU_FALLBACK")]
fn test_budget_macro_dynamic_env_fallback() {
    let budget_env_resolve = |_var: &str| -> Option<String> { None };
    let env = Env::default();
    let client = setup_wasm_client(&env);

    client.do_expensive_work(&10_000);
}
