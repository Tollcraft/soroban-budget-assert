//! Local-vs-network cost-gap measurement for token transfers (issue #415).
//!
//! A token transfer is the single most common expensive operation in deployed
//! Soroban contracts. It is a compound operation — a cross-contract call into
//! the token contract plus storage writes on both sides — so its cost cannot
//! be inferred by adding up parts and is measured directly here. Single
//! transfer plus transfers in a loop across three counts, since batching
//! behaviour (constant vs. growing per-transfer cost) is the practically
//! important question.
//!
//! The token is the SDK's built-in Stellar Asset Contract
//! (`Env::register_stellar_asset_contract_v2`), minted to the pool contract
//! before the measured calls.
//!
//! # Usage
//!
//! ```bash
//! cargo build --target wasm32v1-none --release -p amm-pool-contract
//! cargo test -p amm-pool-contract --test measure_token_transfer_gap -- --nocapture
//! ```
//!
//! Network figures: `simulateTransaction` against Soroban testnet with the
//! same WASM and a deployed SAC; see
//! `cargo-budget-report/fixtures/token_transfer_benchmark.json`.

#![cfg(not(feature = "sdk20"))]

use amm_pool_contract::ConstantProductPoolClient;
use soroban_sdk::{testutils::Address as _, token::StellarAssetClient, Address, Env};

const WASM_PATH: &str = "../target/wasm32v1-none/release/amm_pool_contract.wasm";
const TRANSFER_COUNTS: [u32; 4] = [1, 5, 20, 50];

#[test]
fn measure_token_transfer_gap() {
    let mut per_transfer: Vec<(u32, u64)> = Vec::new();

    for count in TRANSFER_COUNTS {
        let env = Env::default();
        env.mock_all_auths();

        let wasm = std::fs::read(WASM_PATH)
            .expect("WASM not found — run cargo build --target wasm32v1-none --release -p amm-pool-contract");
        let pool_id = env.register(wasm.as_slice(), ());
        let pool = ConstantProductPoolClient::new(&env, &pool_id);

        let admin = Address::generate(&env);
        let sac = env.register_stellar_asset_contract_v2(admin.clone());
        let token = sac.address();
        StellarAssetClient::new(&env, &token).mint(&pool_id, &1_000_000_000i128);

        let recipient = Address::generate(&env);

        env.cost_estimate().budget().reset_unlimited();
        pool.do_token_transfers(&token, &recipient, &1i128, &count);
        let b = env.cost_estimate().budget();
        let cpu = b.cpu_instruction_cost();
        let mem = b.memory_bytes_cost();

        println!("=== TOKEN_TRANSFER_MEASUREMENT count={count} ===");
        println!("TRANSFER_{count}_CPU={cpu}");
        println!("TRANSFER_{count}_MEM={mem}");
        per_transfer.push((count, cpu));
    }

    // Per-transfer cost: constant or growing with batch size?
    println!("--- marginal CPU per transfer (from the count deltas) ---");
    for pair in per_transfer.windows(2) {
        let (c0, cpu0) = pair[0];
        let (c1, cpu1) = pair[1];
        let marginal = (cpu1 - cpu0) / u64::from(c1 - c0);
        println!("PER_TRANSFER_CPU[{c0}..{c1}]={marginal}");
    }
}
