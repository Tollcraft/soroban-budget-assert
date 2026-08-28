#[cfg(test)]
mod measure_map_gap {
    use host_function_contract::HostFunctionBenchmarkClient;
    use soroban_sdk::Env;

    /// Registers the WASM in a fresh `Env`, resets its budget to unlimited,
    /// runs `call_fn`, and returns the resulting cumulative CPU instruction
    /// cost. The WASM is re-registered per invocation so measurements reflect
    /// a clean module instantiation, matching how the network figure is
    /// captured by `simulateTransaction`.
    fn measure_cpu(call_fn: impl FnOnce(&HostFunctionBenchmarkClient<'_>)) -> u64 {
        let env = Env::default();
        let wasm_path = "../target/wasm32v1-none/release/host_function_contract.wasm";
        let wasm = std::fs::read(wasm_path).expect("WASM file not found, did you run cargo build?");
        let contract_id = env.register(wasm.as_slice(), ());
        let client = HostFunctionBenchmarkClient::new(&env, &contract_id);

        env.cost_estimate().budget().reset_unlimited();

        call_fn(&client);

        env.cost_estimate().budget().cpu_instruction_cost()
    }

    #[test]
    fn measure_map_insert_across_sizes() {
        for &size in &[100u32, 500, 1_000] {
            let cpu = measure_cpu(|c| {
                c.map_insert(&size);
            });
            println!("INSERT size={:>5} local WASM CPU: {:>12}", size, cpu);
        }
    }

    #[test]
    fn measure_map_get_across_sizes() {
        for &size in &[100u32, 500, 1_000] {
            let cpu = measure_cpu(|c| {
                c.map_get(&size);
            });
            println!("GET size={:>5} local WASM CPU: {:>12}", size, cpu);
        }
    }

    #[test]
    fn measure_map_remove_across_sizes() {
        for &size in &[100u32, 500, 1_000] {
            let cpu = measure_cpu(|c| {
                c.map_remove(&size);
            });
            println!("REMOVE size={:>5} local WASM CPU: {:>12}", size, cpu);
        }
    }

    #[test]
    fn measure_map_iterate_across_sizes() {
        for &size in &[100u32, 500, 1_000] {
            let cpu = measure_cpu(|c| {
                c.map_iterate(&size);
            });
            println!("ITERATE size={:>5} local WASM CPU: {:>12}", size, cpu);
        }
    }
}
