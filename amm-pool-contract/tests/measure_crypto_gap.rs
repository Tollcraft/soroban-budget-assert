//! Local-vs-network cost-gap measurement for cryptographic host functions
//! (issue #414).
//!
//! Hashing and signature verification are among the most expensive host calls
//! and the ones most likely to push a single invocation over its CPU budget.
//! They are also where the local estimate has particular reason to mislead:
//! locally they run as native Rust, on the network through the host's metered
//! implementation.
//!
//! # Usage
//!
//! ```bash
//! cargo build --target wasm32v1-none --release -p amm-pool-contract
//! cargo test -p amm-pool-contract --test measure_crypto_gap -- --nocapture
//! ```
//!
//! Each `*_CPU` / `*_MEM` line is a WASM-registered local estimate from
//! `Env::cost_estimate().budget()`. The network figure for each row requires a
//! `simulateTransaction` run against Soroban testnet with the same WASM; see
//! `cargo-budget-report/fixtures/crypto_operations_benchmark.json`.

#![cfg(not(feature = "sdk20"))]

use amm_pool_contract::ConstantProductPoolClient;
use soroban_sdk::{Bytes, BytesN, Env};

const WASM_PATH: &str = "../target/wasm32v1-none/release/amm_pool_contract.wasm";

fn client(env: &Env) -> ConstantProductPoolClient<'_> {
    let wasm = std::fs::read(WASM_PATH).expect(
        "WASM not found — run cargo build --target wasm32v1-none --release -p amm-pool-contract",
    );
    let id = env.register(wasm.as_slice(), ());
    ConstantProductPoolClient::new(env, &id)
}

fn measure(env: &Env, call: impl FnOnce()) -> (u64, u64) {
    env.cost_estimate().budget().reset_unlimited();
    call();
    let b = env.cost_estimate().budget();
    (b.cpu_instruction_cost(), b.memory_bytes_cost())
}

#[test]
fn measure_crypto_gap() {
    // Hashing across three input sizes: does the gap scale with message length?
    for size in [64usize, 1024, 8192] {
        let env = Env::default();
        let c = client(&env);
        let data = Bytes::from_slice(&env, &vec![0xA5u8; size]);
        let (cpu, mem) = measure(&env, || {
            c.hash_sha256(&data);
        });
        println!("=== CRYPTO_MEASUREMENT sha256/{size}B ===");
        println!("SHA256_{size}_CPU={cpu}");
        println!("SHA256_{size}_MEM={mem}");
    }

    for size in [64usize, 1024, 8192] {
        let env = Env::default();
        let c = client(&env);
        let data = Bytes::from_slice(&env, &vec![0x5Au8; size]);
        let (cpu, mem) = measure(&env, || {
            c.hash_keccak256(&data);
        });
        println!("=== CRYPTO_MEASUREMENT keccak256/{size}B ===");
        println!("KECCAK256_{size}_CPU={cpu}");
        println!("KECCAK256_{size}_MEM={mem}");
    }

    // ed25519 verification. Invoked through `try_` so an arbitrary 64-byte
    // value does not abort: the host runs the full scalar multiplication before
    // it can accept or reject, so the metered cost is representative of a real
    // verification regardless of the outcome. A 32-byte message is used.
    {
        let env = Env::default();
        let c = client(&env);
        let key = BytesN::<32>::from_array(&env, &[7u8; 32]);
        let msg = Bytes::from_slice(&env, &[9u8; 32]);
        let sig = BytesN::<64>::from_array(&env, &[3u8; 64]);
        let (cpu, mem) = measure(&env, || {
            let _ = c.try_verify_ed25519(&key, &msg, &sig);
        });
        println!("=== CRYPTO_MEASUREMENT ed25519_verify/32B ===");
        println!("ED25519_VERIFY_CPU={cpu}");
        println!("ED25519_VERIFY_MEM={mem}");
    }

    println!("--- crypto functions on soroban_sdk::crypto::Crypto (SDK 22.0.11) ---");
    println!("measured: sha256, keccak256, ed25519_verify");
    println!("not measured: bls12_381 sub-module (own gap series); CryptoHazmat secp256k1_recover / secp256r1_verify (hazmat feature, unavailable to a plain contract build)");
}
