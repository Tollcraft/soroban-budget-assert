#![cfg(test)]

use amm_pool_contract::{ConstantProductPool, ConstantProductPoolClient};
use budget_macros::{budget_cpu_lt, budget_mem_lt};
use soroban_sdk::{testutils::Address as _, Address, Env};

fn setup_wasm(env: &Env) -> (ConstantProductPoolClient<'_>, Address) {
    let wasm_path = "../target/wasm32-unknown-unknown/release/amm_pool_contract.wasm";
    let wasm = std::fs::read(wasm_path).expect("WASM file not found, did you run cargo build?");
    #[allow(deprecated)]
    let contract_id = env.register_contract_wasm(None, wasm.as_slice());
    let client = ConstantProductPoolClient::new(env, &contract_id);

    let user = Address::generate(env);

    client.initialize();

    env.mock_all_auths();

    env.cost_estimate().budget().reset_unlimited();

    (client, user)
}

#[test]
fn test_budget_raw_rust() {
    let env = Env::default();
    let contract_id = env.register(ConstantProductPool, ());
    let client = ConstantProductPoolClient::new(&env, &contract_id);

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
    let (client, user) = setup_wasm(&env);

    client.deposit(&user, &10_000_i128, &10_000_i128);
    client.swap(&user, &true, &100_i128, &90_i128);
    client.withdraw(&user, &1_000_i128, &900_i128, &900_i128);

    let budget = env.cost_estimate().budget();
    println!("=== WASM LOCAL ===");
    println!("CPU instructions: {}", budget.cpu_instruction_cost());
    println!("Memory bytes: {}", budget.memory_bytes_cost());
}

#[test]
#[budget_cpu_lt(2500000)] // Re-measured: WASM local 2307555, simulates deposit+swap+withdraw
fn test_budget_macro_gated() {
    let env = Env::default();
    let (client, user) = setup_wasm(&env);

    client.deposit(&user, &10_000_i128, &10_000_i128);
    client.swap(&user, &true, &100_i128, &90_i128);
    client.withdraw(&user, &1_000_i128, &900_i128, &900_i128);
}

#[test]
#[should_panic(
    expected = "local estimate, real network cost may differ significantly in either direction"
)]
#[budget_cpu_lt(1000000)] // Deliberate regression: AMM pool costs ~2.3M CPU
fn test_budget_macro_deliberate_regression() {
    let env = Env::default();
    let (client, user) = setup_wasm(&env);

    client.deposit(&user, &10_000_i128, &10_000_i128);
    client.swap(&user, &true, &100_i128, &90_i128);
    client.withdraw(&user, &1_000_i128, &900_i128, &900_i128);
}

#[test]
#[should_panic(
    expected = "local estimate, real network cost may differ significantly in either direction"
)]
#[budget_mem_lt(1)] // Deliberate regression: any real memory cost exceeds an impossible 1-byte limit
fn test_budget_macro_mem_deliberate_regression() {
    let env = Env::default();
    let (client, user) = setup_wasm(&env);

    client.deposit(&user, &10_000_i128, &10_000_i128);
    client.swap(&user, &true, &100_i128, &90_i128);
    client.withdraw(&user, &1_000_i128, &900_i128, &900_i128);
}

#[test]
#[budget_cpu_lt(env = "TEST_MAX_CPU")]
fn test_budget_macro_dynamic_env() {
    let budget_env_resolve = |var: &str| -> Option<String> {
        if var == "TEST_MAX_CPU" {
            Some("2500000".to_string())
        } else {
            None
        }
    };
    let env = Env::default();
    let (client, user) = setup_wasm(&env);

    client.deposit(&user, &10_000_i128, &10_000_i128);
    client.swap(&user, &true, &100_i128, &90_i128);
    client.withdraw(&user, &1_000_i128, &900_i128, &900_i128);
}

#[test]
#[budget_cpu_lt(env = "TEST_MAX_CPU_FALLBACK")]
fn test_budget_macro_dynamic_env_fallback() {
    let budget_env_resolve = |_var: &str| -> Option<String> { None };
    let env = Env::default();
    let (client, user) = setup_wasm(&env);

    client.deposit(&user, &10_000_i128, &10_000_i128);
    client.swap(&user, &true, &100_i128, &90_i128);
    client.withdraw(&user, &1_000_i128, &900_i128, &900_i128);
}

#[test]
#[should_panic(expected = "budget_cpu_lt: env var BAD_CPU_LIMIT")]
#[budget_cpu_lt(env = "BAD_CPU_LIMIT")]
fn test_budget_macro_dynamic_env_invalid_value() {
    let budget_env_resolve = |var: &str| -> Option<String> {
        if var == "BAD_CPU_LIMIT" {
            Some("1_000_000".to_string())
        } else {
            None
        }
    };
    let env = Env::default();
    let contract_id = env.register(ConstantProductPool, ());
    let client = ConstantProductPoolClient::new(&env, &contract_id);
    env.cost_estimate().budget().reset_unlimited();
    client.do_expensive_work(&10_000);
}

#[test]
#[should_panic(expected = "budget_mem_lt: env var BAD_MEM_LIMIT")]
#[budget_mem_lt(env = "BAD_MEM_LIMIT")]
fn test_budget_macro_mem_dynamic_env_invalid_value() {
    let budget_env_resolve = |var: &str| -> Option<String> {
        if var == "BAD_MEM_LIMIT" {
            Some("not_a_number".to_string())
        } else {
            None
        }
    };
    let env = Env::default();
    let contract_id = env.register(ConstantProductPool, ());
    let client = ConstantProductPoolClient::new(&env, &contract_id);
    env.cost_estimate().budget().reset_unlimited();
    client.do_expensive_work(&10_000);
}

/// Fixture: contract invocation stays within the read bytes budget.
///
/// Runs a deposit + swap + withdraw cycle against the WASM contract and
/// asserts that the ledger read bytes reported by
/// `env.cost_estimate().resources().read_bytes` do not exceed a generous
/// upper bound.  This test is expected to pass under normal conditions and
/// acts as a regression guard that will fail if storage reads grow
/// unexpectedly.
#[test]
fn test_read_bytes_budget_within_limit() {
    let env = Env::default();
    let (client, user) = setup_wasm(&env);

    client.deposit(&user, &10_000_i128, &10_000_i128);
    client.swap(&user, &true, &100_i128, &90_i128);
    client.withdraw(&user, &1_000_i128, &900_i128, &900_i128);

    let read_bytes = env.cost_estimate().resources().read_bytes;
    println!("Read bytes (WASM deposit+swap+withdraw): {read_bytes}");

    // Generous upper bound (measured ~16,252 on CI) — tighten once a clean baseline is recorded.
    assert!(
        read_bytes < 20_000,
        "Read bytes {read_bytes} exceeded the expected limit of 20,000 \
         - local estimate, real network cost may differ significantly in either direction"
    );
}

/// Fixture: deliberate regression — contract exceeds the read bytes budget.
///
/// Sets an impossibly tight read bytes limit (1 byte) to demonstrate what a
/// read-bytes budget breach looks like.  The `#[should_panic]` attribute
/// documents the expected failure message so that the test suite treats this
/// as a passing regression fixture rather than a real failure.
#[test]
#[should_panic(
    expected = "local estimate, real network cost may differ significantly in either direction"
)]
fn test_read_bytes_budget_exceeds_limit() {
    let env = Env::default();
    let (client, user) = setup_wasm(&env);

    client.deposit(&user, &10_000_i128, &10_000_i128);
    client.swap(&user, &true, &100_i128, &90_i128);
    client.withdraw(&user, &1_000_i128, &900_i128, &900_i128);

    let read_bytes = env.cost_estimate().resources().read_bytes;
    println!("Read bytes (deliberate regression): {read_bytes}");

    // Deliberately impossible limit: any real WASM invocation will read more
    // than 1 byte from ledger storage, so this assertion always fires.
    let limit: u32 = 1;
    assert!(
        read_bytes < limit,
        "Read bytes {read_bytes} exceeded the expected limit of {limit} \
         - local estimate, real network cost may differ significantly in either direction"
    );
}
