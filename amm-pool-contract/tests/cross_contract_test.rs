#![cfg(test)]

use amm_pool_contract::{ConstantProductPool, ConstantProductPoolClient, HelperContract};
use budget_macros::budget_cpu_lt;
use soroban_sdk::{Address, Env};

#[test]
fn test_cross_contract_raw_rust() {
    let env = Env::default();
    let helper_address: Address = env.register(HelperContract, ());
    let contract_address: Address = env.register(ConstantProductPool, ());
    let client = ConstantProductPoolClient::new(&env, &contract_address);

    env.cost_estimate().budget().reset_unlimited();

    client.do_cross_contract_work(&helper_address, &10_000);

    let budget = env.cost_estimate().budget();
    println!("=== CROSS-CONTRACT RAW RUST ===");
    println!("CPU instructions: {}", budget.cpu_instruction_cost());
    println!("Memory bytes: {}", budget.memory_bytes_cost());
}

#[test]
fn test_cross_contract_wasm() {
    let env = Env::default();

    let wasm_path = "../target/wasm32v1-none/release/amm_pool_contract.wasm";
    let wasm = std::fs::read(wasm_path).expect("WASM file not found, did you run cargo build?");
    #[allow(deprecated)]
    let helper_address: Address = env.register_contract_wasm(None, wasm.as_slice());
    #[allow(deprecated)]
    let contract_address: Address = env.register_contract_wasm(None, wasm.as_slice());
    let client = ConstantProductPoolClient::new(&env, &contract_address);

    env.cost_estimate().budget().reset_unlimited();

    client.do_cross_contract_work(&helper_address, &100);

    let budget = env.cost_estimate().budget();
    println!("=== CROSS-CONTRACT WASM ===");
    println!("CPU instructions: {}", budget.cpu_instruction_cost());
    println!("Memory bytes: {}", budget.memory_bytes_cost());
}

#[test]
#[budget_cpu_lt(300000000)]
fn test_cross_contract_macro_gated() {
    let env = Env::default();

    let wasm_path = "../target/wasm32v1-none/release/amm_pool_contract.wasm";
    let wasm = std::fs::read(wasm_path).expect("WASM file not found, did you run cargo build?");
    #[allow(deprecated)]
    let helper_address: Address = env.register_contract_wasm(None, wasm.as_slice());
    #[allow(deprecated)]
    let contract_address: Address = env.register_contract_wasm(None, wasm.as_slice());
    let client = ConstantProductPoolClient::new(&env, &contract_address);

    env.cost_estimate().budget().reset_unlimited();

    client.do_cross_contract_work(&helper_address, &100);
}

#[test]
fn test_cross_contract_address_raw_rust() {
    let env = Env::default();
    let contract_address: Address = env.register(ConstantProductPool, ());
    let helper_address: Address = env.register(HelperContract, ());
    let client = ConstantProductPoolClient::new(&env, &contract_address);

    env.cost_estimate().budget().reset_unlimited();

    client.do_cross_contract_work(&helper_address, &5_000);

    let budget = env.cost_estimate().budget();
    println!("=== CROSS-CONTRACT SMALL N ===");
    println!("CPU instructions: {}", budget.cpu_instruction_cost());
    println!("Memory bytes: {}", budget.memory_bytes_cost());
}
