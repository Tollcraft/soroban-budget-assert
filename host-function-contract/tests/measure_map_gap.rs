//! Local-vs-network cost-gap measurement for Soroban `Map` host operations
//! (issue #715).
//!
//! Inserts, lookups, removals, and iteration over a `Map<u32, u32>` are all
//! billed by the host, but at different per-operation rates. This harness
//! registers the fixture WASM in a fresh environment for each measurement and
//! records the CPU instruction cost of a single `map_*` call, so the local
//! estimate can be compared against the matching `simulateTransaction` figure
//! recorded in `MEASUREMENTS.md`.
//!
//! Each non-insert operation first performs the same `size`-entry build that
//! the `map_insert` fixture performs, so the marginal per-operation cost is
//! `(map_<op>(size) - map_insert(size)) / size`.
//!
//! # Running
//!
//! ```bash
//! cargo build --target wasm32v1-none --release -p host-function-contract
//! cargo test -p host-function-contract --test measure_map_gap -- --nocapture
//! ```

mod common;

use host_function_contract::HostFunctionBenchmarkClient;
use soroban_sdk::Env;

/// Map sizes measured for every operation. Small enough that the whole series
/// runs quickly, large enough to expose the difference between the
/// constant-time-per-call operations and the super-linear `remove`.
const SIZES: [u32; 3] = [100, 500, 1_000];

/// Registers `wasm` in a fresh [`Env`], resets its budget to unlimited, runs
/// `call_fn`, and returns the resulting cumulative CPU instruction cost.
///
/// A fresh `Env` — and therefore a clean module instantiation — is used per
/// measurement so the local figure is captured the same way the network figure
/// is captured by `simulateTransaction`.
///
/// The already-loaded `wasm` bytes are passed in by the caller rather than
/// read here, so the artifact is read from disk once per test instead of once
/// per measured size. The staleness check and build-on-demand live in
/// [`common::load_contract_wasm`].
fn measure_cpu(wasm: &[u8], call_fn: impl FnOnce(&HostFunctionBenchmarkClient<'_>)) -> u64 {
    let env = Env::default();
    let contract_id = env.register(wasm, ());
    let client = HostFunctionBenchmarkClient::new(&env, &contract_id);

    env.cost_estimate().budget().reset_unlimited();

    call_fn(&client);

    env.cost_estimate().budget().cpu_instruction_cost()
}

#[test]
fn measure_map_insert_across_sizes() {
    let wasm = common::load_contract_wasm("wasm32v1-none");
    for &size in &SIZES {
        let cpu = measure_cpu(&wasm, |c| {
            c.map_insert(&size);
        });
        println!("INSERT size={:>5} local WASM CPU: {:>12}", size, cpu);
    }
}

#[test]
fn measure_map_get_across_sizes() {
    let wasm = common::load_contract_wasm("wasm32v1-none");
    for &size in &SIZES {
        let cpu = measure_cpu(&wasm, |c| {
            c.map_get(&size);
        });
        println!("GET size={:>5} local WASM CPU: {:>12}", size, cpu);
    }
}

#[test]
fn measure_map_remove_across_sizes() {
    let wasm = common::load_contract_wasm("wasm32v1-none");
    for &size in &SIZES {
        let cpu = measure_cpu(&wasm, |c| {
            c.map_remove(&size);
        });
        println!("REMOVE size={:>5} local WASM CPU: {:>12}", size, cpu);
    }
}

#[test]
fn measure_map_iterate_across_sizes() {
    let wasm = common::load_contract_wasm("wasm32v1-none");
    for &size in &SIZES {
        let cpu = measure_cpu(&wasm, |c| {
            c.map_iterate(&size);
        });
        println!("ITERATE size={:>5} local WASM CPU: {:>12}", size, cpu);
    }
}
