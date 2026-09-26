//! Integration tests for the host-function measurement fixture (issues #480, #716).
//!
//! The fixture exists so host-function costs can be measured against
//! something small and predictable: `repeated_sequence` repeatedly calls the
//! `ledger().sequence()` host function and returns the final value. It is the
//! benchmark operation documented in `host-function-contract/README.md` and
//! used by the host-function-call row in `MEASUREMENTS.md`, so it is not dead
//! fixture code.
//!
//! This file exercises **every** entry point on the fixture, not just
//! `repeated_sequence`, and covers the zero/empty edge case of each (#716):
//!
//! * ledger reads — `repeated_sequence`, `repeated_timestamp`;
//! * crypto — `repeated_hash`;
//! * allocation — `repeated_bytes_new`;
//! * Map operations — `map_insert`, `map_get`, `map_remove`, `map_iterate`.
//!
//! Every assertion is run twice: once against the contract registered as
//! native Rust (`env.register`) and once against the contract registered from
//! its built `wasm32v1-none` artifact (`register_contract_wasm`), so the
//! fixture is exercised at the same WASM level the rest of the workspace uses
//! for budget measurement, not just as a native `cargo test`.
//!
//! The tests assert on the documented return values (iteration/size counts and
//! the identity-sum from `map_iterate`). They deliberately keep iteration
//! counts and map sizes small: this file checks correctness, while the
//! `measure_*` harnesses produce the cost figures.

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

/// Sum returned by `map_iterate(size)`: the map is seeded with identity entries
/// `i -> i` for `i` in `0..size`, so the total is `size * (size - 1) / 2`.
fn identity_sum(size: u32) -> u32 {
    (u64::from(size) * u64::from(size).saturating_sub(1) / 2) as u32
}

// ── Native registration: ledger reads ─────────────────────────────────────

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

#[test]
fn repeated_timestamp_returns_the_current_ledger_timestamp() {
    let env = Env::default();
    let client = native_client(&env);

    let expected = env.ledger().timestamp();
    assert_eq!(client.repeated_timestamp(&1_000), expected);
}

#[test]
fn repeated_timestamp_with_zero_iterations_returns_zero() {
    let env = Env::default();
    let client = native_client(&env);

    assert_eq!(client.repeated_timestamp(&0), 0);
}

#[test]
fn repeated_timestamp_is_deterministic_across_iteration_counts() {
    let env = Env::default();
    let client = native_client(&env);

    // Reading the timestamp does not advance the ledger, so every iteration
    // count must observe the same value.
    let expected = env.ledger().timestamp();
    for iterations in [1, 10, 1_000, 10_000] {
        assert_eq!(client.repeated_timestamp(&iterations), expected);
    }
}

// ── Native registration: crypto and allocation ────────────────────────────

#[test]
fn repeated_hash_returns_the_iteration_count() {
    let env = Env::default();
    let client = native_client(&env);

    for iterations in [0, 1, 10, 100] {
        assert_eq!(client.repeated_hash(&iterations), iterations);
    }
}

#[test]
fn repeated_hash_with_zero_iterations_returns_zero() {
    let env = Env::default();
    let client = native_client(&env);

    // No digest is computed, and the counter stays at its initial value.
    assert_eq!(client.repeated_hash(&0), 0);
}

#[test]
fn repeated_bytes_new_returns_the_iteration_count() {
    let env = Env::default();
    let client = native_client(&env);

    for iterations in [0, 1, 10, 100] {
        assert_eq!(client.repeated_bytes_new(&iterations), iterations);
    }
}

#[test]
fn repeated_bytes_new_with_zero_iterations_returns_zero() {
    let env = Env::default();
    let client = native_client(&env);

    assert_eq!(client.repeated_bytes_new(&0), 0);
}

// ── Native registration: Map operations ───────────────────────────────────

#[test]
fn map_insert_returns_the_number_of_inserts() {
    let env = Env::default();
    let client = native_client(&env);

    for size in [0, 1, 5, 100] {
        assert_eq!(client.map_insert(&size), size);
    }
}

#[test]
fn map_insert_with_zero_size_returns_zero() {
    let env = Env::default();
    let client = native_client(&env);

    assert_eq!(client.map_insert(&0), 0);
}

#[test]
fn map_get_returns_the_number_of_lookups() {
    let env = Env::default();
    let client = native_client(&env);

    for size in [0, 1, 5, 100] {
        assert_eq!(client.map_get(&size), size);
    }
}

#[test]
fn map_get_with_zero_size_returns_zero() {
    let env = Env::default();
    let client = native_client(&env);

    assert_eq!(client.map_get(&0), 0);
}

#[test]
fn map_remove_returns_the_number_of_removals() {
    let env = Env::default();
    let client = native_client(&env);

    for size in [0, 1, 5, 100] {
        assert_eq!(client.map_remove(&size), size);
    }
}

#[test]
fn map_remove_with_zero_size_returns_zero() {
    let env = Env::default();
    let client = native_client(&env);

    assert_eq!(client.map_remove(&0), 0);
}

#[test]
fn map_iterate_returns_the_sum_of_all_values() {
    let env = Env::default();
    let client = native_client(&env);

    for size in [0, 1, 5, 10] {
        assert_eq!(client.map_iterate(&size), identity_sum(size));
    }
}

#[test]
fn map_iterate_with_empty_map_returns_zero() {
    let env = Env::default();
    let client = native_client(&env);

    assert_eq!(client.map_iterate(&0), 0);
}

#[test]
fn map_iterate_with_single_zero_valued_entry_returns_zero() {
    let env = Env::default();
    let client = native_client(&env);

    // The only entry is `0 -> 0`, so the accumulator remains at zero.
    assert_eq!(client.map_iterate(&1), 0);
}

// ── WASM registration: ledger reads ───────────────────────────────────────

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

#[test]
fn wasm_repeated_timestamp_returns_the_current_ledger_timestamp() {
    let env = Env::default();
    let client = wasm_client(&env);

    let expected = env.ledger().timestamp();
    assert_eq!(client.repeated_timestamp(&1_000), expected);
}

#[test]
fn wasm_repeated_timestamp_with_zero_iterations_returns_zero() {
    let env = Env::default();
    let client = wasm_client(&env);

    assert_eq!(client.repeated_timestamp(&0), 0);
}

// ── WASM registration: crypto and allocation ──────────────────────────────

#[test]
fn wasm_repeated_hash_returns_the_iteration_count() {
    let env = Env::default();
    let client = wasm_client(&env);

    for iterations in [0, 1, 10, 100] {
        assert_eq!(client.repeated_hash(&iterations), iterations);
    }
}

#[test]
fn wasm_repeated_bytes_new_returns_the_iteration_count() {
    let env = Env::default();
    let client = wasm_client(&env);

    for iterations in [0, 1, 10, 100] {
        assert_eq!(client.repeated_bytes_new(&iterations), iterations);
    }
}

// ── WASM registration: Map operations ─────────────────────────────────────

#[test]
fn wasm_map_insert_returns_the_number_of_inserts() {
    let env = Env::default();
    let client = wasm_client(&env);

    for size in [0, 1, 5, 100] {
        assert_eq!(client.map_insert(&size), size);
    }
}

#[test]
fn wasm_map_get_returns_the_number_of_lookups() {
    let env = Env::default();
    let client = wasm_client(&env);

    for size in [0, 1, 5, 100] {
        assert_eq!(client.map_get(&size), size);
    }
}

#[test]
fn wasm_map_remove_returns_the_number_of_removals() {
    let env = Env::default();
    let client = wasm_client(&env);

    for size in [0, 1, 5, 100] {
        assert_eq!(client.map_remove(&size), size);
    }
}

#[test]
fn wasm_map_iterate_returns_the_sum_of_all_values() {
    let env = Env::default();
    let client = wasm_client(&env);

    for size in [0, 1, 5, 10] {
        assert_eq!(client.map_iterate(&size), identity_sum(size));
    }
}

#[test]
fn wasm_map_operations_with_zero_size_return_zero() {
    let env = Env::default();
    let client = wasm_client(&env);

    // Every Map entry point must accept an empty map without erroring.
    assert_eq!(client.map_insert(&0), 0);
    assert_eq!(client.map_get(&0), 0);
    assert_eq!(client.map_remove(&0), 0);
    assert_eq!(client.map_iterate(&0), 0);
}
