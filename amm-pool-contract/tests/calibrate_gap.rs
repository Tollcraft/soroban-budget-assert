// @measure local  # discovered by scripts/regenerate-measurements.sh
#![cfg(not(feature = "sdk20"))]

//! Calibration test for cost estimation gap measurement (baseline SDK version).
//!
//! This harness measures the local CPU and memory cost estimates for the
//! synthetic `do_expensive_work` operation at a 10k loop depth, establishing a
//! local-vs-network baseline gap. The figures are used to calibrate real contract
//! costs against the empirical local estimates.
//!
//! # Usage
//!
//! This test is marked `#[ignore]` and excluded from the default test suite.
//! To run it deliberately:
//!
//! ```bash
//! cargo build --target wasm32v1-none --release -p amm-pool-contract
//! cargo test -p amm-pool-contract --test calibrate_gap -- --ignored --nocapture
//! ```
//!
//! # Output
//!
//! The test prints `CALIBRATE_CPU=<value>` and `CALIBRATE_MEM=<value>` to stdout.
//! These values are manually transcribed into `MEASUREMENTS.md` under the
//! "Local Cost Estimates" section as baseline figures.

#[cfg(test)]
mod calibrate_gap {
    use amm_pool_contract::ConstantProductPoolClient;
    use soroban_sdk::Env;

    /// Reads the pre-built WASM artifact bytes from disk.
    fn load_wasm_bytes() -> Vec<u8> {
        let wasm_path = "../target/wasm32v1-none/release/amm_pool_contract.wasm";
        std::fs::read(wasm_path).expect("WASM file not found, did you run cargo build?")
    }

    /// Measures the CPU instruction and memory byte cost of `do_expensive_work(10_000)`
    /// in the given environment.
    ///
    /// Returns `(cpu_instructions, memory_bytes)`.
    fn measure_cpu_mem(env: &Env, wasm: &[u8]) -> (u64, u64) {
        let contract_id = env.register(wasm, ());
        let client = ConstantProductPoolClient::new(env, &contract_id);

        env.mock_all_auths();
        env.cost_estimate().budget().reset_unlimited();

        client.do_expensive_work(&10_000);

        let budget = env.cost_estimate().budget();
        (budget.cpu_instruction_cost(), budget.memory_bytes_cost())
    }

    /// Prints calibration results in the format expected by
    /// `scripts/regenerate-measurements.sh`.
    fn print_calibration_results(cpu: u64, mem: u64) {
        println!("=== CALIBRATE_GAP ===");
        println!("CALIBRATE_CPU={}", cpu);
        println!("CALIBRATE_MEM={}", mem);
    }

    fn measure_do_expensive_work(env: &Env) {
        let wasm = load_wasm_bytes();
        let (cpu, mem) = measure_cpu_mem(env, &wasm);
        print_calibration_results(cpu, mem);
    }

    #[test]
    #[ignore]
    fn calibrate_gap() {
        let env = Env::default();
        measure_do_expensive_work(&env);
    }

    /// Verifies that `measure_cpu_mem` returns non-zero costs.
    ///
    /// Any real WASM invocation consumes measurable CPU and memory; a zero
    /// reading indicates the budget was not reset or the measurement is broken.
    #[test]
    #[ignore]
    fn calibrate_gap_measurements_are_nonzero() {
        let env = Env::default();
        let wasm = load_wasm_bytes();
        let (cpu, mem) = measure_cpu_mem(&env, &wasm);
        assert!(cpu > 0, "CPU cost must be non-zero for do_expensive_work");
        assert!(
            mem > 0,
            "memory cost must be non-zero for do_expensive_work"
        );
    }

    /// Verifies that two independent measurements of the same workload in the
    /// same process return consistent results (both non-zero, same order of
    /// magnitude).
    ///
    /// This guards against accidental accumulated-state measurements where the
    /// second env sees a budget dirtied by the first.
    #[test]
    #[ignore]
    fn calibrate_gap_repeated_measurement_is_consistent() {
        let wasm = load_wasm_bytes();

        let env1 = Env::default();
        let (cpu1, mem1) = measure_cpu_mem(&env1, &wasm);

        let env2 = Env::default();
        let (cpu2, mem2) = measure_cpu_mem(&env2, &wasm);

        assert!(cpu1 > 0 && cpu2 > 0, "both CPU costs must be non-zero");
        assert!(mem1 > 0 && mem2 > 0, "both memory costs must be non-zero");

        // Same module, same workload — costs must be identical across fresh envs.
        assert_eq!(
            cpu1, cpu2,
            "CPU cost must be deterministic across fresh Env instances"
        );
        assert_eq!(
            mem1, mem2,
            "memory cost must be deterministic across fresh Env instances"
        );
    }
}
