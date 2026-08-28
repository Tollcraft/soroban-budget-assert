#![cfg(not(feature = "sdk20"))]

//! Local WASM measurement for the storage-write gap series in
//! `MEASUREMENTS.md` ("## Storage writes"). Measures `do_write_persistent`,
//! `do_write_temporary`, and `do_write_instance` at several entry counts so
//! the gap stability across input sizes can be reported alongside the
//! local-vs-network delta.
//!
//! # Running
//!
//! ```bash
//! cargo build --target wasm32v1-none --release -p amm-pool-contract
//! cargo test -p amm-pool-contract --test measure_storage_write_gap -- --nocapture
//! ```

#[cfg(test)]
mod measure_storage_write_gap {
    use amm_pool_contract::ConstantProductPoolClient;
    use soroban_sdk::Env;

    const ENTRY_COUNTS: [u32; 3] = [10, 50, 100];

    fn load_wasm() -> Vec<u8> {
        let wasm_path = "../target/wasm32v1-none/release/amm_pool_contract.wasm";
        std::fs::read(wasm_path).expect("WASM file not found, did you run cargo build?")
    }

    #[test]
    fn measure_storage_write_persistent() {
        let wasm = load_wasm();
        println!("=== STORAGE_WRITE_PERSISTENT_WASM_LOCAL ===");
        for &n in &ENTRY_COUNTS {
            let env = Env::default();
            let contract_id = env.register(wasm.as_slice(), ());
            let client = ConstantProductPoolClient::new(&env, &contract_id);
            env.cost_estimate().budget().reset_unlimited();

            client.do_write_persistent(&n);

            let budget = env.cost_estimate().budget();
            let cpu = budget.cpu_instruction_cost();
            let mem = budget.memory_bytes_cost();
            println!(
                "n={:>6} CPU_INSTRUCTIONS={:>10} MEMORY_BYTES={:>10}",
                n, cpu, mem
            );
        }
    }

    #[test]
    fn measure_storage_write_temporary() {
        let wasm = load_wasm();
        println!("=== STORAGE_WRITE_TEMPORARY_WASM_LOCAL ===");
        for &n in &ENTRY_COUNTS {
            let env = Env::default();
            let contract_id = env.register(wasm.as_slice(), ());
            let client = ConstantProductPoolClient::new(&env, &contract_id);
            env.cost_estimate().budget().reset_unlimited();

            client.do_write_temporary(&n);

            let budget = env.cost_estimate().budget();
            let cpu = budget.cpu_instruction_cost();
            let mem = budget.memory_bytes_cost();
            println!(
                "n={:>6} CPU_INSTRUCTIONS={:>10} MEMORY_BYTES={:>10}",
                n, cpu, mem
            );
        }
    }

    #[test]
    fn measure_storage_write_instance() {
        let wasm = load_wasm();
        println!("=== STORAGE_WRITE_INSTANCE_WASM_LOCAL ===");
        for &n in &ENTRY_COUNTS {
            let env = Env::default();
            let contract_id = env.register(wasm.as_slice(), ());
            let client = ConstantProductPoolClient::new(&env, &contract_id);
            env.cost_estimate().budget().reset_unlimited();

            client.do_write_instance(&n);

            let budget = env.cost_estimate().budget();
            let cpu = budget.cpu_instruction_cost();
            let mem = budget.memory_bytes_cost();
            println!(
                "n={:>6} CPU_INSTRUCTIONS={:>10} MEMORY_BYTES={:>10}",
                n, cpu, mem
            );
        }
    }
}
