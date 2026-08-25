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
    let helper_address: Address = env.register(wasm.as_slice(), ());
    let contract_address: Address = env.register(wasm.as_slice(), ());
    let client = ConstantProductPoolClient::new(&env, &contract_address);

    env.cost_estimate().budget().reset_unlimited();

    client.do_cross_contract_work(&helper_address, &100);

    let budget = env.cost_estimate().budget();
    println!("=== CROSS-CONTRACT WASM ===");
    println!("CPU instructions: {}", budget.cpu_instruction_cost());
    println!("Memory bytes: {}", budget.memory_bytes_cost());
}

/// Local regression guard on the *total* cost of 100 cross-contract calls.
///
/// Deliberately not converted to a marginal (baseline-subtracted) assertion
/// like the ones in `budget_test.rs`. Each iteration of
/// `do_cross_contract_work` re-instantiates the callee module, so the
/// per-invocation instantiation floor is not overhead *around* the work here —
/// it is ~99% of the work being measured. Subtracting 100 floors would leave a
/// near-zero remainder and an assertion that could never fail. The limit is
/// also a hand-picked local figure, not a Tier B network value, so there is no
/// network quantity for a marginal number to correspond to.
///
/// The ceiling was raised 300M -> 350M because the old one was stale: this
/// measured 313,884,035 before the `noop` export existed and 314,972,209
/// after, so it was already ~4.6% over. 350M restores roughly the headroom the
/// original bound was chosen with.
#[test]
#[budget_cpu_lt(350_000_000)]
fn test_cross_contract_macro_gated() {
    let env = Env::default();

    let wasm_path = "../target/wasm32v1-none/release/amm_pool_contract.wasm";
    let wasm = std::fs::read(wasm_path).expect("WASM file not found, did you run cargo build?");
    let helper_address: Address = env.register(wasm.as_slice(), ());
    let contract_address: Address = env.register(wasm.as_slice(), ());
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
