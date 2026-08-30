// @measure local  # discovered by scripts/regenerate-measurements.sh
#![cfg(not(feature = "sdk20"))]

//! Calibration test for `require_auth()` cost gap measurement.
//!
//! Measures the local CPU and memory cost estimates for a single `require_auth()`
//! call on a generated address, establishing a local-vs-network gap for
//! authorization operations.
//!
//! # Usage
//!
//! This test is marked `#[ignore]` and excluded from the default test suite.
//! To run it deliberately:
//!
//! ```bash
//! cargo build --target wasm32v1-none --release -p amm-pool-contract
//! cargo test -p amm-pool-contract --test measure_auth_gap -- --ignored --nocapture
//! ```
//!
//! # Output
//!
//! The test prints `AUTH_CPU=<value>` and `AUTH_MEM=<value>` to stdout.
//! These values are manually transcribed into `MEASUREMENTS.md` under the
//! "Authorization Cost" section.

#[cfg(test)]
mod measure_auth_gap {
    use amm_pool_contract::ConstantProductPoolClient;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    fn measure_require_auth_only(env: &Env) {
        let wasm_path = "../target/wasm32v1-none/release/amm_pool_contract.wasm";
        let wasm = std::fs::read(wasm_path).expect("WASM file not found, did you run cargo build?");
        let contract_id = env.register(wasm.as_slice(), ());
        let client = ConstantProductPoolClient::new(env, &contract_id);

        let user = Address::generate(env);

        env.mock_all_auths();
        env.cost_estimate().budget().reset_unlimited();

        client.require_auth_only(&user);

        let budget = env.cost_estimate().budget();
        let cpu = budget.cpu_instruction_cost();
        let mem = budget.memory_bytes_cost();

        println!("=== REQUIRE_AUTH_MEASUREMENT ===");
        println!("AUTH_CPU={}", cpu);
        println!("AUTH_MEM={}", mem);
    }

    #[test]
    #[ignore]
    fn measure_auth_gap() {
        let env = Env::default();
        measure_require_auth_only(&env);
    }
}
