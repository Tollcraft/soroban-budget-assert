//! Local-vs-network cost-gap measurement for cross-contract call depth
//! (issue #416).
//!
//! `cross_contract_test.rs` measures the single-hop case. This measures what
//! happens deeper: a contract calling a contract that calls a third, and so
//! on. Each hop carries its own dispatch, Val-conversion and authorization
//! overhead; whether that accumulates linearly is the finding.
//!
//! The chain is built from `RelayContract` (a new fixture in
//! `amm-pool-contract/src/lib.rs`, not a change to the existing test): N
//! instances of the same WASM are registered and instances `2..=N` are passed
//! as the `chain` to instance 1, so a call into instance 1 reaches call
//! depth N.
//!
//! # Usage
//!
//! ```bash
//! cargo build --target wasm32v1-none --release -p amm-pool-contract
//! cargo test -p amm-pool-contract --test measure_call_depth_gap -- --nocapture
//! ```
//!
//! Network figures: `simulateTransaction` against Soroban testnet with N
//! deployed instances; see
//! `cargo-budget-report/fixtures/cross_contract_depth_benchmark.json`.

#![cfg(not(feature = "sdk20"))]

use amm_pool_contract::RelayContractClient;
use soroban_sdk::{Address, Env, Vec as SdkVec};

const WASM_PATH: &str = "../target/wasm32v1-none/release/amm_pool_contract.wasm";
const DEPTHS: [u32; 4] = [1, 2, 3, 4];

#[test]
fn measure_call_depth_gap() {
    let mut points: std::vec::Vec<(u32, u64)> = std::vec::Vec::new();

    for depth in DEPTHS {
        let env = Env::default();
        let wasm = std::fs::read(WASM_PATH)
            .expect("WASM not found — run cargo build --target wasm32v1-none --release -p amm-pool-contract");

        // Register `depth` instances of the relay contract.
        let ids: std::vec::Vec<Address> = (0..depth)
            .map(|_| env.register(wasm.as_slice(), ()))
            .collect();
        let head = RelayContractClient::new(&env, &ids[0]);

        // The chain the head relays through: instances 2..=depth.
        let mut chain: SdkVec<Address> = SdkVec::new(&env);
        for id in ids.iter().skip(1) {
            chain.push_back(id.clone());
        }

        env.cost_estimate().budget().reset_unlimited();
        let reached = head.relay(&chain, &0u32);
        let b = env.cost_estimate().budget();
        let cpu = b.cpu_instruction_cost();
        let mem = b.memory_bytes_cost();

        // `relay` increments once per additional frame it enters; the head
        // frame is depth 1, so it reports `depth - 1`.
        assert_eq!(reached, depth - 1, "relay should report the hops it took");
        println!("=== CALL_DEPTH_MEASUREMENT depth={depth} ===");
        println!("DEPTH_{depth}_CPU={cpu}");
        println!("DEPTH_{depth}_MEM={mem}");
        points.push((depth, cpu));
    }

    println!("--- marginal CPU per additional hop ---");
    for pair in points.windows(2) {
        let (d0, cpu0) = pair[0];
        let (d1, cpu1) = pair[1];
        let per_hop = (cpu1 as i64 - cpu0 as i64) / i64::from(d1 - d0);
        println!("PER_HOP_CPU[{d0}..{d1}]={per_hop}");
    }
}
