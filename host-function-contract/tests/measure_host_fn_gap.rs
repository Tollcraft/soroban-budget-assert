#[cfg(test)]
mod measure_host_fn_gap {
    use host_function_contract::HostFunctionBenchmarkClient;
    use soroban_sdk::Env;

    /// Registers the WASM in a fresh `Env`, resets its budget to unlimited,
    /// runs `call_fn`, and returns the resulting cumulative CPU instruction
    /// cost. The WASM is re-registered per invocation so measurements reflect
    /// a clean module instantiation, matching how the network figure is
    /// captured by `simulateTransaction`.
    fn measure_cpu(call_fn: impl FnOnce(&HostFunctionBenchmarkClient<'_>)) -> (u64, u64) {
        let env = Env::default();
        let wasm_path = "../target/wasm32v1-none/release/host_function_contract.wasm";
        let wasm = std::fs::read(wasm_path).expect("WASM file not found, did you run cargo build?");
        let contract_id = env.register(wasm.as_slice(), ());
        let client = HostFunctionBenchmarkClient::new(&env, &contract_id);

        env.mock_all_auths();
        env.cost_estimate().budget().reset_unlimited();

        call_fn(&client);

        let budget = env.cost_estimate().budget();
        (budget.cpu_instruction_cost(), budget.memory_bytes_cost())
    }

    #[test]
    fn measure_host_fn_sequence_1000() {
        let (cpu, mem) = measure_cpu(|c| {
            c.repeated_sequence(&1_000);
        });
        println!("=== SEQUENCE_1000 ===");
        println!("SEQUENCE_1000_CPU={}", cpu);
        println!("SEQUENCE_1000_MEM={}", mem);
    }

    #[test]
    fn measure_host_fn_timestamp_1000() {
        let (cpu, mem) = measure_cpu(|c| {
            c.repeated_timestamp(&1_000);
        });
        println!("=== TIMESTAMP_1000 ===");
        println!("TIMESTAMP_1000_CPU={}", cpu);
        println!("TIMESTAMP_1000_MEM={}", mem);
    }

    #[test]
    fn measure_host_fn_hash_1000() {
        let (cpu, mem) = measure_cpu(|c| {
            c.repeated_hash(&1_000);
        });
        println!("=== HASH_1000 ===");
        println!("HASH_1000_CPU={}", cpu);
        println!("HASH_1000_MEM={}", mem);
    }

    #[test]
    fn measure_host_fn_bytes_new_1000() {
        let (cpu, mem) = measure_cpu(|c| {
            c.repeated_bytes_new(&1_000);
        });
        println!("=== BYTES_NEW_1000 ===");
        println!("BYTES_NEW_1000_CPU={}", cpu);
        println!("BYTES_NEW_1000_MEM={}", mem);
    }

    #[test]
    fn measure_gap_stability_across_call_counts() {
        let call_counts = [0, 100, 1_000, 5_000, 10_000];

        for &n in &call_counts {
            let (cpu, _mem) = measure_cpu(|c| {
                c.repeated_sequence(&n);
            });
            println!("n={:>6} | repeated_sequence local WASM CPU: {:>10}", n, cpu);
        }
    }
}
