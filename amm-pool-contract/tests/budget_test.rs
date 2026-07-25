#![cfg(test)]

use amm_pool_contract::{ConstantProductPool, ConstantProductPoolClient};
use budget_macros::{budget_cpu_lt, budget_mem_lt};
use soroban_sdk::{testutils::Address as _, Address, Env};

fn setup_wasm(env: &Env) -> (ConstantProductPoolClient<'_>, Address) {
    let wasm_path = "../target/wasm32-unknown-unknown/release/amm_pool_contract.wasm";
    let wasm = std::fs::read(wasm_path).expect("WASM file not found, did you run cargo build?");
    // AUDIT (Issue #92): `soroban_sdk::Env::register_contract_wasm` is deprecated in soroban-sdk 22.x
    // in favor of `Env::register`. However, `Env::register` only registers Rust contract types for
    // in-memory host execution, whereas `register_contract_wasm` remains the sole API in soroban-sdk 22.x
    // for registering raw precompiled `.wasm` byte slices into the test environment VM. Because WASM-level
    // execution is required for accurate CPU/memory budget measurements (raw Rust estimates undercount costs),
    // `register_contract_wasm` with `#[allow(deprecated)]` remains necessary until soroban-sdk provides
    // a non-deprecated replacement for raw WASM byte registration.
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
