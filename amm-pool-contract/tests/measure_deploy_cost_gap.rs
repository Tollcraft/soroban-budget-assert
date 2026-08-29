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

#[test]
fn measure_deploy_cost_gap() {
    let mut points: std::vec::Vec<(usize, u64)> = std::vec::Vec::new();

    for (name, path) in CONTRACTS {
        let wasm = std::fs::read(path).unwrap_or_else(|_| {
            panic!(
                "{path} not found — run: cargo build --release --target wasm32v1-none \
                 -p amm-pool-contract -p host-function-contract -p bloat-contract"
            )
        });
        let bytes = wasm.len();

        let env = Env::default();
        env.cost_estimate().budget().reset_unlimited();
        let _id = env.register(wasm.as_slice(), ());
        let b = env.cost_estimate().budget();
        let cpu = b.cpu_instruction_cost();
        let mem = b.memory_bytes_cost();

        println!("=== DEPLOY_COST_MEASUREMENT {name} ===");
        println!("DEPLOY_{name}_WASM_BYTES={bytes}");
        println!("DEPLOY_{name}_CPU={cpu}");
        println!("DEPLOY_{name}_MEM={mem}");
        points.push((bytes, cpu));
    }

    println!("--- CPU per wasm byte, between adjacent size points ---");
    for pair in points.windows(2) {
        let (b0, cpu0) = pair[0];
        let (b1, cpu1) = pair[1];
        let per_byte = (cpu1 as i64 - cpu0 as i64) as f64 / (b1 as i64 - b0 as i64) as f64;
        println!("PER_BYTE_CPU[{b0}..{b1}]={per_byte:.1}");
    }
}
