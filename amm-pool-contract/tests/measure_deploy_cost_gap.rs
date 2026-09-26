//! Local-vs-network cost-gap measurement for wasm size vs. deploy cost
//! (issue #417).
//!
//! Every other measurement here concerns what a contract costs to *run*. This
//! one concerns what it costs to *deploy* — a function of compiled size, and
//! subject to its own network limit (`maxContractSizeBytes`).
//!
//! `Env::register(&[u8], ())` drives the same host path a real deploy does —
//! `upload_contract_wasm` + `CreateContractV2`, no constructor args (verified
//! against soroban-sdk 22.0.11, see `budget_test.rs`) — so the budget cost
//! recorded immediately after `register`, before any invocation, is the local
//! estimate of deploy cost.
//!
//! Three contracts of materially different compiled size feed the measurement:
//! `host-function-contract` (~0.8 KB), `bloat-contract` (~14 KB) and
//! `amm-pool-contract` (~30 KB). All are real contracts — size comes from real
//! code, since padding compresses differently under `opt-level = "z"` + LTO
//! and would misrepresent the relationship.
//!
//! The **release profile matters**: the workspace `[profile.release]` sets
//! `opt-level = "z"`, `lto = true`, `strip = "symbols"`, `codegen-units = 1`.
//! An unoptimised build would not reflect what is deployed, so all three wasms
//! must be built `--release --target wasm32v1-none`.
//!
//! # Usage
//!
//! ```bash
//! cargo build --release --target wasm32v1-none \
//!   -p amm-pool-contract -p host-function-contract -p bloat-contract
//! cargo test -p amm-pool-contract --test measure_deploy_cost_gap -- --nocapture
//! ```
//!
//! Network figures: `simulateTransaction` on the upload/create operations
//! against Soroban testnet, plus the live `maxContractSizeBytes` from
//! `getNetworkLimits` (the RPC call `cargo-budget-report` already uses). See
//! `cargo-budget-report/fixtures/deploy_cost_benchmark.json`.

#![cfg(not(feature = "sdk20"))]

use soroban_sdk::Env;

const CONTRACTS: [(&str, &str); 3] = [
    (
        "host-function-contract",
        "../target/wasm32v1-none/release/host_function_contract.wasm",
    ),
    (
        "bloat-contract",
        "../target/wasm32v1-none/release/bloat_contract.wasm",
    ),
    (
        "amm-pool-contract",
        "../target/wasm32v1-none/release/amm_pool_contract.wasm",
    ),
];

/// Reads a WASM artifact from `path`.
///
/// Panics with a clear build command if the file is missing.
fn load_wasm(path: &str, name: &str) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|_| {
        panic!(
            "{path} not found — run: cargo build --release --target wasm32v1-none \
             -p amm-pool-contract -p host-function-contract -p bloat-contract \
             (missing: {name})"
        )
    })
}

/// Measures the deploy cost (CPU instructions, memory bytes) for `wasm_bytes`
/// by registering them in a fresh `Env` and reading the budget immediately
/// after — before any contract invocation.
///
/// The WASM bytes are moved in so they are dropped before the budget is read,
/// avoiding a needless live allocation during the measurement window.
fn measure_deploy_cost(wasm_bytes: Vec<u8>) -> (usize, u64, u64) {
    let byte_count = wasm_bytes.len();
    let env = Env::default();
    env.cost_estimate().budget().reset_unlimited();
    // `wasm_bytes` is consumed here and released from the caller's stack.
    let _id = env.register(wasm_bytes.as_slice(), ());
    drop(wasm_bytes);
    let b = env.cost_estimate().budget();
    (byte_count, b.cpu_instruction_cost(), b.memory_bytes_cost())
}

/// Prints deploy cost measurements for one contract in the format expected by
/// `scripts/regenerate-measurements.sh`.
fn print_deploy_measurement(name: &str, bytes: usize, cpu: u64, mem: u64) {
    println!("=== DEPLOY_COST_MEASUREMENT {name} ===");
    println!("DEPLOY_{name}_WASM_BYTES={bytes}");
    println!("DEPLOY_{name}_CPU={cpu}");
    println!("DEPLOY_{name}_MEM={mem}");
}

/// Prints the incremental CPU-per-byte slope between adjacent measurement
/// points in the size series.
fn print_per_byte_slopes(points: &[(usize, u64)]) {
    println!("--- CPU per wasm byte, between adjacent size points ---");
    for pair in points.windows(2) {
        let (b0, cpu0) = pair[0];
        let (b1, cpu1) = pair[1];
        let per_byte = (cpu1 as i64 - cpu0 as i64) as f64 / (b1 as i64 - b0 as i64) as f64;
        println!("PER_BYTE_CPU[{b0}..{b1}]={per_byte:.1}");
    }
}

#[test]
fn measure_deploy_cost_gap() {
    // Pre-allocate to avoid reallocations as points are collected.
    let mut points: Vec<(usize, u64)> = Vec::with_capacity(CONTRACTS.len());

    for (name, path) in CONTRACTS {
        let wasm = load_wasm(path, name);
        let (bytes, cpu, mem) = measure_deploy_cost(wasm);
        print_deploy_measurement(name, bytes, cpu, mem);
        points.push((bytes, cpu));
    }

    print_per_byte_slopes(&points);
}

/// Verifies that every measured contract produces non-zero CPU and memory costs.
///
/// A zero reading would indicate the budget was not reset before registration
/// or the measurement helpers are broken.
#[test]
fn deploy_cost_measurements_are_nonzero() {
    for (name, path) in CONTRACTS {
        let wasm = load_wasm(path, name);
        let (bytes, cpu, mem) = measure_deploy_cost(wasm);
        assert!(bytes > 0, "{name}: WASM byte count must be > 0");
        assert!(cpu > 0, "{name}: deploy CPU cost must be > 0");
        assert!(mem > 0, "{name}: deploy memory cost must be > 0");
    }
}

/// Verifies that larger contracts have higher (or equal) CPU deploy costs.
///
/// The host's `upload_contract_wasm` cost scales with input size, so a larger
/// WASM module must not be cheaper to deploy than a smaller one.
#[test]
fn deploy_cpu_cost_increases_with_wasm_size() {
    let measurements: Vec<(usize, u64)> = CONTRACTS
        .iter()
        .map(|(name, path)| {
            let wasm = load_wasm(path, name);
            let (bytes, cpu, _mem) = measure_deploy_cost(wasm);
            (bytes, cpu)
        })
        .collect();

    // Contracts are listed small-to-large; verify the ordering holds.
    for pair in measurements.windows(2) {
        let (b0, cpu0) = pair[0];
        let (b1, cpu1) = pair[1];
        assert!(
            b1 >= b0,
            "test assumption violated: contracts are not ordered small-to-large \
             ({b0} bytes then {b1} bytes)"
        );
        assert!(
            cpu1 >= cpu0,
            "deploy CPU cost must be non-decreasing with WASM size: \
             {b0}-byte contract cost {cpu0}, but {b1}-byte contract cost only {cpu1}"
        );
    }
}
