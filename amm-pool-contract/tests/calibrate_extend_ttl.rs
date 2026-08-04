//! Calibration test for TTL extension budget measurement.
//!
//! This test follows the same pattern as `calibrate_gap.rs`: it registers the
//! contract as WASM, calls the target function (`extend_instance_ttl`), and
//! prints the local CPU and memory budget estimates so they can be compared
//! against a network-verified `simulateTransaction` figure.
//!
//! # Usage
//!
//! ```bash
//! cargo build --target wasm32v1-none --release -p amm-pool-contract
//! cargo test -p amm-pool-contract --test calibrate_extend_ttl -- --nocapture
//! ```
//!
//! The network figure requires a separate `cargo-budget-report` run on Soroban
//! testnet against the same WASM.

#![cfg(not(feature = "sdk20"))]

use amm_pool_contract::ConstantProductPoolClient;
use soroban_sdk::Env;

fn measure_extend_ttl(env: &Env) {
    let wasm_path = "../target/wasm32v1-none/release/amm_pool_contract.wasm";
    let wasm = std::fs::read(wasm_path).expect("WASM file not found, did you run cargo build?");
    // AUDIT (Issue #92): `register_contract_wasm` is deprecated but remains the
    // only API for registering raw WASM bytes in soroban-sdk 22.x.
    #[allow(deprecated)]
    let contract_id = env.register_contract_wasm(None, wasm.as_slice());
    let client = ConstantProductPoolClient::new(env, &contract_id);

    env.mock_all_auths();
    env.cost_estimate().budget().reset_unlimited();

    // Initialize so instance storage entries exist before extending TTL.
    client.initialize();

    // Extend TTL: threshold = 100 ledgers, extend_to = 10,000 ledgers.
    // These values match the existing test_budget_extend_ttl_isolated so the
    // measurement is comparable to the budget assertion test.
    client.extend_instance_ttl(&100, &10_000);

    let budget = env.cost_estimate().budget();
    let cpu = budget.cpu_instruction_cost();
    let mem = budget.memory_bytes_cost();

    println!("=== CALIBRATE_EXTEND_TTL ===");
    println!("CALIBRATE_CPU={}", cpu);
    println!("CALIBRATE_MEM={}", mem);
}

#[test]
fn calibrate_extend_ttl() {
    let env = Env::default();
    measure_extend_ttl(&env);
}
