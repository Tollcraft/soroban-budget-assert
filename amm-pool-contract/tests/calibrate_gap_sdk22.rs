// @measure local:sdk22  # discovered by scripts/regenerate-measurements.sh
#![cfg(feature = "sdk22")]

//! Calibration test for cost estimation gap measurement (Soroban SDK 22.x).
//!
//! This harness measures the local CPU and memory cost estimates for the
//! synthetic `do_expensive_work` operation at a 10k loop depth under SDK 22.x,
//! establishing a local-vs-network baseline gap. The figures are used to
//! calibrate real contract costs against the empirical local estimates.
//!
//! # Usage
//!
//! This test is marked `#[ignore]` and excluded from the default test suite.
//! To run it deliberately:
//!
//! ```bash
//! cargo build --target wasm32v1-none --release -p amm-pool-contract --features sdk22
//! cargo test -p amm-pool-contract --test calibrate_gap_sdk22 --features sdk22 -- --ignored --nocapture
//! ```
//!
//! # Output
//!
//! The test prints `CALIBRATE_CPU=<value>` and `CALIBRATE_MEM=<value>` to stdout.
//! These values are manually transcribed into `MEASUREMENTS.md` under the
//! "Local Cost Estimates (SDK 22.x)" section.

#[cfg(test)]
mod calibrate_gap {
    use amm_pool_contract::ConstantProductPoolClient;
    use soroban_sdk::Env;

    fn measure_do_expensive_work(env: &Env) {
        let wasm_path = "../target/wasm32v1-none/release/amm_pool_contract.wasm";
        let wasm = std::fs::read(wasm_path).expect("WASM file not found, did you run cargo build?");
        let contract_id = env.register(wasm.as_slice(), ());
        let client = ConstantProductPoolClient::new(env, &contract_id);

        env.mock_all_auths();
        env.cost_estimate().budget().reset_unlimited();

        client.do_expensive_work(&10_000);

        let budget = env.cost_estimate().budget();
        let cpu = budget.cpu_instruction_cost();
        let mem = budget.memory_bytes_cost();

        println!("=== CALIBRATE_GAP ===");
        println!("CALIBRATE_CPU={}", cpu);
        println!("CALIBRATE_MEM={}", mem);
    }

    #[test]
    #[ignore]
    fn calibrate_gap() {
        let env = Env::default();
        measure_do_expensive_work(&env);
    }
}
