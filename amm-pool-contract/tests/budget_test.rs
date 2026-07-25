#![cfg(test)]

use std::sync::{Mutex, PoisonError};

use amm_pool_contract::{ConstantProductPool, ConstantProductPoolClient};
use budget_macros::{budget_cpu_lt, budget_mem_lt};
use soroban_sdk::{testutils::Address as _, Address, Env};

/// Serialises all JSON-config tests so they never read stale `budget.json`
/// content written by another test running in parallel.
static BUDGET_JSON_LOCK: Mutex<()> = Mutex::new(());

/// A Drop guard that writes `budget.json` on creation and removes it on drop
/// (including during stack unwinding from a panic). The `_lock` field prevents
/// other JSON-config tests from overwriting the file while the assertion runs.
struct BudgetJsonGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl BudgetJsonGuard {
    fn create(content: &str) -> Self {
        let lock = BUDGET_JSON_LOCK
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        std::fs::write("budget.json", content).expect("failed to write budget.json");
        BudgetJsonGuard { _lock: lock }
    }
}

impl Drop for BudgetJsonGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file("budget.json");
    }
}

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
#[budget_cpu_lt(2500000)]
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
#[budget_cpu_lt(1000000)]
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
#[budget_mem_lt(1)]
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

// ---------------------------------------------------------------------------
// JSON config tests
// ---------------------------------------------------------------------------

#[test]
#[budget_cpu_lt(config = "cpu_instructions")]
fn test_budget_macro_json_config_valid() {
    let _guard = BudgetJsonGuard::create(r#"{"cpu_instructions": 2500000}"#);
    let env = Env::default();
    let (client, user) = setup_wasm(&env);

    client.deposit(&user, &10_000_i128, &10_000_i128);
    client.swap(&user, &true, &100_i128, &90_i128);
    client.withdraw(&user, &1_000_i128, &900_i128, &900_i128);
}

#[test]
#[budget_mem_lt(config = "memory_bytes")]
fn test_budget_macro_json_config_mem_valid() {
    let _guard = BudgetJsonGuard::create(r#"{"memory_bytes": 5000000}"#);
    let env = Env::default();
    let (client, user) = setup_wasm(&env);

    client.deposit(&user, &10_000_i128, &10_000_i128);
    client.swap(&user, &true, &100_i128, &90_i128);
    client.withdraw(&user, &1_000_i128, &900_i128, &900_i128);
}

#[test]
#[should_panic(expected = "key 'non_existent_key' not found or invalid in budget.json")]
#[budget_cpu_lt(config = "non_existent_key")]
fn test_budget_macro_json_config_missing_key() {
    let _guard = BudgetJsonGuard::create(r#"{"some_other_key": 100}"#);
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
#[budget_cpu_lt(config = "cpu_instructions_deliberate")]
fn test_budget_macro_json_config_deliberate_regression() {
    let _guard = BudgetJsonGuard::create(r#"{"cpu_instructions_deliberate": 1}"#);
    let env = Env::default();
    let (client, user) = setup_wasm(&env);

    client.deposit(&user, &10_000_i128, &10_000_i128);
    client.swap(&user, &true, &100_i128, &90_i128);
    client.withdraw(&user, &1_000_i128, &900_i128, &900_i128);
}

#[test]
#[should_panic(expected = "key 'cpu_instructions' not found or invalid in budget.json")]
#[budget_cpu_lt(config = "cpu_instructions")]
fn test_budget_macro_json_config_missing_key_empty_config() {
    // Empty JSON object -> requested key won't be found -> macro panics.
    let _guard = BudgetJsonGuard::create(r#"{}"#);
    let env = Env::default();
    let (client, user) = setup_wasm(&env);

    client.deposit(&user, &10_000_i128, &10_000_i128);
    client.swap(&user, &true, &100_i128, &90_i128);
    client.withdraw(&user, &1_000_i128, &900_i128, &900_i128);
}

#[test]
#[should_panic(expected = "key 'cpu_instructions' not found or invalid in budget.json")]
#[budget_cpu_lt(config = "cpu_instructions")]
fn test_budget_macro_json_config_invalid_json() {
    let _guard = BudgetJsonGuard::create(r#"this is not valid json at all"#);
    let env = Env::default();
    let (client, user) = setup_wasm(&env);

    client.deposit(&user, &10_000_i128, &10_000_i128);
    client.swap(&user, &true, &100_i128, &90_i128);
    client.withdraw(&user, &1_000_i128, &900_i128, &900_i128);
}
