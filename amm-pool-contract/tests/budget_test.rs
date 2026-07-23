#![cfg(test)]

use amm_pool_contract::{ExpensiveContract, ExpensiveContractClient};
use budget_macros::{budget_cpu_lt, budget_write_bytes_lt};
use soroban_sdk::Env;

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
    let wasm_path = "../target/wasm32-unknown-unknown/release/amm_pool_contract.wasm";
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
#[budget_cpu_lt(950000)] // Re-measured with the Soroban release profile applied: WASM local 901816, actual testnet ~756678
fn test_budget_macro_gated() {
    let env = Env::default();

    // Path to the compiled wasm
    let wasm_path = "../target/wasm32-unknown-unknown/release/amm_pool_contract.wasm";
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
    std::env::set_var("TEST_MAX_CPU", "950000");
    let env = Env::default();

    let wasm_path = "../target/wasm32-unknown-unknown/release/amm_pool_contract.wasm";
    let wasm = std::fs::read(wasm_path).expect("WASM file not found, did you run cargo build?");
    #[allow(deprecated)]
    let contract_id = env.register_contract_wasm(None, wasm.as_slice());
    let client = ExpensiveContractClient::new(&env, &contract_id);

    env.cost_estimate().budget().reset_unlimited();

    client.do_expensive_work(&10_000);
}

#[test]
#[budget_cpu_lt(env = "TEST_MAX_CPU_FALLBACK")]
fn test_budget_macro_dynamic_env_fallback() {
    std::env::remove_var("TEST_MAX_CPU_FALLBACK");
    let env = Env::default();

    let wasm_path = "../target/wasm32-unknown-unknown/release/amm_pool_contract.wasm";
    let wasm = std::fs::read(wasm_path).expect("WASM file not found, did you run cargo build?");
    #[allow(deprecated)]
    let contract_id = env.register_contract_wasm(None, wasm.as_slice());
    let client = ExpensiveContractClient::new(&env, &contract_id);

    env.cost_estimate().budget().reset_unlimited();

    client.do_expensive_work(&10_000);
}

// ---------------------------------------------------------------------------
// Write-bytes fixtures
//
// These tests exercise the `do_write_heavy_work` contract function, which
// writes many large byte blobs into temporary storage.  The local
// `memory_bytes_cost` is used as a proxy for ledger write bytes because the
// actual on-network write-bytes figure is only available via RPC simulation
// (see `cargo-budget-report`).  These tests document expected cost levels and
// catch regressions in the contract's storage footprint.
// ---------------------------------------------------------------------------

/// Prints the raw memory-bytes cost of a write-heavy invocation so developers
/// can calibrate assertion thresholds.
#[test]
fn test_write_bytes_raw() {
    let env = Env::default();
    let contract_id = env.register(ExpensiveContract, ());
    let client = ExpensiveContractClient::new(&env, &contract_id);

    env.cost_estimate().budget().reset_unlimited();

    client.do_write_heavy_work(&50);

    let budget = env.cost_estimate().budget();
    println!("=== WRITE-HEAVY RAW (n=50) ===");
    println!("CPU instructions:  {}", budget.cpu_instruction_cost());
    println!(
        "Memory bytes (proxy for write bytes): {}",
        budget.memory_bytes_cost()
    );
}

/// Asserts that the write-bytes proxy stays below a generous threshold so
/// normal write-heavy usage passes in CI.
#[test]
#[budget_write_bytes_lt(5_000_000)]
fn test_write_bytes_budget_passes() {
    let env = Env::default();
    let contract_id = env.register(ExpensiveContract, ());
    let client = ExpensiveContractClient::new(&env, &contract_id);

    env.cost_estimate().budget().reset_unlimited();

    // n=50 entries × 256 bytes each = ~12 800 bytes of ledger writes.
    // The memory proxy will be above that but well under 5 000 000.
    client.do_write_heavy_work(&50);
}

/// Demonstrates a deliberate write-bytes regression: the limit is set below
/// the actual cost so the assertion fires and the test panics (as expected).
#[test]
#[should_panic(expected = "local estimate, underestimates real network cost")]
#[budget_write_bytes_lt(1)] // Unrealistically tight limit — always exceeded
fn test_write_bytes_budget_regression() {
    let env = Env::default();
    let contract_id = env.register(ExpensiveContract, ());
    let client = ExpensiveContractClient::new(&env, &contract_id);

    env.cost_estimate().budget().reset_unlimited();

    // Even a single entry will exceed a limit of 1 byte.
    client.do_write_heavy_work(&1);
}
