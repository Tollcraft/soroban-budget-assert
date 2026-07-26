#![cfg(test)]

use std::sync::{Mutex, PoisonError};

use amm_pool_contract::{ConstantProductPool, ConstantProductPoolClient};
use budget_macros::{budget_cpu_lt, budget_lt, budget_mem_lt};
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
    let wasm_path = "../target/wasm32v1-none/release/amm_pool_contract.wasm";
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
#[budget_cpu_lt(5000000)]
fn test_budget_wasm() {
    let env = Env::default();
    let (client, user) = setup_wasm(&env);

    client.deposit(&user, &10_000_i128, &10_000_i128);
    client.swap(&user, &true, &100_i128, &90_i128);
    client.withdraw(&user, &1_000_i128, &900_i128, &900_i128);
}

#[test]
#[budget_lt(cpu = 50000000, mem = 50000000)]
fn test_budget_require_auth_deposit() {
    let env = Env::default();
    let (client, user) = setup_wasm(&env);

    client.deposit(&user, &10_000_i128, &10_000_i128);
}

#[test]
#[budget_cpu_lt(50000000)]
fn test_budget_require_auth_swap() {
    let env = Env::default();
    let (client, user) = setup_wasm(&env);

    client.deposit(&user, &10_000_i128, &10_000_i128);
    client.swap(&user, &true, &100_i128, &90_i128);
}

#[test]
#[budget_cpu_lt(50000000)]
fn test_budget_require_auth_withdraw() {
    let env = Env::default();
    let (client, user) = setup_wasm(&env);

    client.deposit(&user, &10_000_i128, &10_000_i128);
    client.swap(&user, &true, &100_i128, &90_i128);
    client.withdraw(&user, &1_000_i128, &900_i128, &900_i128);
}

#[test]
#[budget_cpu_lt(50000000)]
fn test_budget_require_auth_isolated() {
    let env = Env::default();
    let (client, user) = setup_wasm(&env);

    client.require_auth_only(&user);
}

#[test]
#[budget_mem_lt(2000000)]
fn test_budget_require_auth_isolated_mem() {
    let env = Env::default();
    let (client, user) = setup_wasm(&env);

    client.require_auth_only(&user);
}

#[test]
#[should_panic(
    expected = "local estimate, real network cost may differ significantly in either direction"
)]
#[budget_cpu_lt(1000)] // Deliberate regression: require_auth costs well above 1K CPU
fn test_budget_require_auth_deliberate_regression_cpu() {
    let env = Env::default();
    let (client, user) = setup_wasm(&env);

    client.require_auth_only(&user);
}

#[test]
#[should_panic(
    expected = "local estimate, real network cost may differ significantly in either direction"
)]
#[budget_mem_lt(1)] // Deliberate regression: any real memory cost exceeds an impossible 1-byte limit
fn test_budget_require_auth_deliberate_regression_mem() {
    let env = Env::default();
    let (client, user) = setup_wasm(&env);

    client.require_auth_only(&user);
}

#[test]
#[budget_cpu_lt(50000000)]
fn test_budget_extend_ttl_isolated() {
    let env = Env::default();
    let (client, _user) = setup_wasm(&env);

    client.extend_instance_ttl(&100, &10_000);
}

#[test]
#[budget_mem_lt(2000000)]
fn test_budget_extend_ttl_isolated_mem() {
    let env = Env::default();
    let (client, _user) = setup_wasm(&env);

    client.extend_instance_ttl(&100, &10_000);
}

#[test]
#[should_panic(
    expected = "local estimate, real network cost may differ significantly in either direction"
)]
#[budget_cpu_lt(1000)] // Deliberate regression: extend_ttl costs well above 1K CPU
fn test_budget_extend_ttl_deliberate_regression_cpu() {
    let env = Env::default();
    let (client, _user) = setup_wasm(&env);

    client.extend_instance_ttl(&100, &10_000);
}

#[test]
#[should_panic(
    expected = "local estimate, real network cost may differ significantly in either direction"
)]
#[budget_mem_lt(1)] // Deliberate regression: any real memory cost exceeds an impossible 1-byte limit
fn test_budget_extend_ttl_deliberate_regression_mem() {
    let env = Env::default();
    let (client, _user) = setup_wasm(&env);

    client.extend_instance_ttl(&100, &10_000);
}

#[test]
#[budget_cpu_lt(3000000)] // Re-measured: WASM local 2770850, simulates deposit+swap+withdraw
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
            Some("3000000".to_string())
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

/// Fixture: env-var form of the macro actually enforces the limit.
///
/// The two env-var tests above prove that a passing value passes and that
/// an unset variable defaults to `u64::MAX` (passing unconditionally).
/// Neither demonstrates the critical property — that when the environment
/// variable returns a value *below* the measured WASM cost, the macro
/// **must** panic.  Without this test, a future refactor of the
/// env-parsing path could silently stop enforcing env-provided limits.
///
/// This test shadows the default `budget_env_resolve` (which reads real
/// process env vars) with a closure that returns `"1"` — an impossibly
/// low CPU budget — so the macro assertion always fires.
#[test]
#[should_panic(
    expected = "local estimate, real network cost may differ significantly in either direction"
)]
#[budget_cpu_lt(env = "BUDGET_ENFORCEMENT_DELIBERATE_FAIL")]
fn test_budget_macro_dynamic_env_deliberate_regression() {
    let budget_env_resolve = |var: &str| -> Option<String> {
        if var == "BUDGET_ENFORCEMENT_DELIBERATE_FAIL" {
            Some("1".to_string())
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

// ---------------------------------------------------------------------------
// JSON config tests
// ---------------------------------------------------------------------------

#[test]
#[budget_cpu_lt(config = "cpu_instructions")]
fn test_budget_macro_json_config_valid() {
    let _guard = BudgetJsonGuard::create(r#"{"cpu_instructions": 3000000}"#);
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

    // Generous upper bound (measured ~20,236 locally) — tighten once a clean baseline is recorded.
    assert!(
        read_bytes < 21_000,
        "Read bytes {read_bytes} exceeded the expected limit of 21,000 \
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
