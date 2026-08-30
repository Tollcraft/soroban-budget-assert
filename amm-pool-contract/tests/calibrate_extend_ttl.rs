// @measure local  # discovered by scripts/regenerate-measurements.sh
//! Calibration test for TTL extension budget measurement.
//!
//! Measures local CPU and memory cost estimates for extending both instance
//! and persistent storage TTLs at three different `extend_to` values so the
//! stability of the local-vs-network gap can be evaluated.
//!
//! # Usage
//!
//! These tests are marked `#[ignore]` and excluded from the default test suite.
//! To run them deliberately:
//!
//! ```bash
//! cargo build --target wasm32v1-none --release -p amm-pool-contract
//! cargo test -p amm-pool-contract --test calibrate_extend_ttl -- --ignored --nocapture
//! ```
//!
//! # Output
//!
//! Each test prints `CALIBRATE_*_EXTEND_TTL extend_to=<value>` with CPU and memory
//! cost lines. These values are manually transcribed into `MEASUREMENTS.md` under
//! the "TTL Extension Costs" section.
//!
//! The network figures require a separate `cargo-budget-report` run on Soroban
//! testnet against the same WASM.

#![cfg(not(feature = "sdk20"))]

use amm_pool_contract::ConstantProductPoolClient;
use soroban_sdk::Env;

const THRESHOLD: u32 = 100;

/// Register the WASM contract, initialize it, and return the client.
fn setup(env: &Env) -> ConstantProductPoolClient<'_> {
    let wasm_path = "../target/wasm32v1-none/release/amm_pool_contract.wasm";
    let wasm = std::fs::read(wasm_path).expect("WASM file not found, did you run cargo build?");
    let contract_id = env.register(wasm.as_slice(), ());
    let client = ConstantProductPoolClient::new(env, &contract_id);
    env.mock_all_auths();
    // Initialize so instance storage entries exist before extending TTL.
    client.initialize();
    client
}

// ── Instance TTL extension ────────────────────────────────────────────

fn measure_instance_extend_ttl(env: &Env, extend_to: u32) -> (u64, u64) {
    let client = setup(env);
    env.cost_estimate().budget().reset_unlimited();
    client.extend_instance_ttl(&THRESHOLD, &extend_to);
    let budget = env.cost_estimate().budget();
    (budget.cpu_instruction_cost(), budget.memory_bytes_cost())
}

#[test]
#[ignore]
fn calibrate_instance_extend_ttl_1000() {
    let env = Env::default();
    let (cpu, mem) = measure_instance_extend_ttl(&env, 1_000);
    println!("=== CALIBRATE_INSTANCE_EXTEND_TTL extend_to=1000 ===");
    println!("CALIBRATE_CPU={}", cpu);
    println!("CALIBRATE_MEM={}", mem);
}

#[test]
#[ignore]
fn calibrate_instance_extend_ttl_10000() {
    let env = Env::default();
    let (cpu, mem) = measure_instance_extend_ttl(&env, 10_000);
    println!("=== CALIBRATE_INSTANCE_EXTEND_TTL extend_to=10000 ===");
    println!("CALIBRATE_CPU={}", cpu);
    println!("CALIBRATE_MEM={}", mem);
}

#[test]
#[ignore]
fn calibrate_instance_extend_ttl_50000() {
    let env = Env::default();
    let (cpu, mem) = measure_instance_extend_ttl(&env, 50_000);
    println!("=== CALIBRATE_INSTANCE_EXTEND_TTL extend_to=50000 ===");
    println!("CALIBRATE_CPU={}", cpu);
    println!("CALIBRATE_MEM={}", mem);
}

// ── Persistent TTL extension ──────────────────────────────────────────

fn measure_persistent_extend_ttl(env: &Env, extend_to: u32) -> (u64, u64) {
    let client = setup(env);
    env.cost_estimate().budget().reset_unlimited();
    client.extend_persistent_ttl(&THRESHOLD, &extend_to);
    let budget = env.cost_estimate().budget();
    (budget.cpu_instruction_cost(), budget.memory_bytes_cost())
}

#[test]
#[ignore]
fn calibrate_persistent_extend_ttl_1000() {
    let env = Env::default();
    let (cpu, mem) = measure_persistent_extend_ttl(&env, 1_000);
    println!("=== CALIBRATE_PERSISTENT_EXTEND_TTL extend_to=1000 ===");
    println!("CALIBRATE_CPU={}", cpu);
    println!("CALIBRATE_MEM={}", mem);
}

#[test]
#[ignore]
fn calibrate_persistent_extend_ttl_10000() {
    let env = Env::default();
    let (cpu, mem) = measure_persistent_extend_ttl(&env, 10_000);
    println!("=== CALIBRATE_PERSISTENT_EXTEND_TTL extend_to=10000 ===");
    println!("CALIBRATE_CPU={}", cpu);
    println!("CALIBRATE_MEM={}", mem);
}

#[test]
#[ignore]
fn calibrate_persistent_extend_ttl_50000() {
    let env = Env::default();
    let (cpu, mem) = measure_persistent_extend_ttl(&env, 50_000);
    println!("=== CALIBRATE_PERSISTENT_EXTEND_TTL extend_to=50000 ===");
    println!("CALIBRATE_CPU={}", cpu);
    println!("CALIBRATE_MEM={}", mem);
}
