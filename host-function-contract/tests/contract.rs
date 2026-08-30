//! Tests for the host-function measurement fixture (issue #480).
//!
//! The fixture exists so host-function costs can be measured against
//! something small and predictable: `repeated_sequence` repeatedly calls the
//! `ledger().sequence()` host function and returns the final value. It is the
//! benchmark operation documented in `host-function-contract/README.md` and
//! used by the host-function-call row in `MEASUREMENTS.md`, so it is not dead
//! fixture code.
//!
//! Every assertion is run twice: once against the contract registered as
//! native Rust (`env.register`) and once against the contract registered from
//! its built `wasm32v1-none` artifact (`register_contract_wasm`), so the
//! fixture is exercised at the same WASM level the rest of the workspace uses
//! for budget measurement, not just as a native `cargo test`.

mod common;

use host_function_contract::{HostFunctionBenchmark, HostFunctionBenchmarkClient};
use soroban_sdk::Env;

/// Registers the fixture as native Rust and returns a client for it.
fn native_client(env: &Env) -> HostFunctionBenchmarkClient<'_> {
    let contract_id = env.register(HostFunctionBenchmark, ());
    HostFunctionBenchmarkClient::new(env, &contract_id)
}

/// Registers the fixture from its built WASM and returns a client for it.
///
/// AUDIT (Issue #92): `soroban_sdk::Env::register_contract_wasm` is deprecated
/// in soroban-sdk 22.x in favor of `Env::register`, but `Env::register` only
/// registers Rust contract types for in-memory host execution, whereas
/// `register_contract_wasm` remains the sole API in soroban-sdk 22.x for
/// registering raw precompiled `.wasm` byte slices into the test environment
/// VM. Because WASM-level execution is what the measurement fixture exists to
/// exercise, `register_contract_wasm` with `#[allow(deprecated)]` remains
/// necessary until soroban-sdk provides a non-deprecated replacement for raw
/// WASM byte registration.
fn wasm_client(env: &Env) -> HostFunctionBenchmarkClient<'_> {
    let wasm = common::load_contract_wasm("wasm32v1-none");
    #[allow(deprecated)]
    let contract_id = env.register_contract_wasm(None, wasm.as_slice());
    HostFunctionBenchmarkClient::new(env, &contract_id)
}

// ── Native registration ───────────────────────────────────────────────────

#[test]
fn repeated_sequence_returns_the_current_ledger_sequence() {
    let env = Env::default();
    let client = native_client(&env);

    let expected = env.ledger().sequence();
    assert_eq!(client.repeated_sequence(&1_000), expected);
}

#[test]
fn repeated_sequence_with_zero_iterations_returns_zero() {
    let env = Env::default();
    let client = native_client(&env);

    // The loop never executes, so the accumulator keeps its initial value.
    assert_eq!(client.repeated_sequence(&0), 0);
}

#[test]
fn repeated_sequence_is_deterministic_across_iteration_counts() {
    let env = Env::default();
    let client = native_client(&env);

    // Reading the ledger sequence does not advance it, so every iteration
    // count must observe the same value.
    let expected = env.ledger().sequence();
    for iterations in [1, 10, 1_000, 10_000] {
        assert_eq!(client.repeated_sequence(&iterations), expected);
    }
}

// ── WASM registration ─────────────────────────────────────────────────────

#[test]
fn wasm_repeated_sequence_returns_the_current_ledger_sequence() {
    let env = Env::default();
    let client = wasm_client(&env);

    let expected = env.ledger().sequence();
    assert_eq!(client.repeated_sequence(&1_000), expected);
}

#[test]
fn wasm_repeated_sequence_with_zero_iterations_returns_zero() {
    let env = Env::default();
    let client = wasm_client(&env);

    assert_eq!(client.repeated_sequence(&0), 0);
}

#[test]
fn wasm_repeated_sequence_is_deterministic_across_iteration_counts() {
    let env = Env::default();
    let client = wasm_client(&env);

    let expected = env.ledger().sequence();
    for iterations in [1, 10, 1_000, 10_000] {
        assert_eq!(client.repeated_sequence(&iterations), expected);
    }
}
