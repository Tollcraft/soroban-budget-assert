// @measure local:sdk20  # discovered by scripts/regenerate-measurements.sh
#![cfg(feature = "sdk20")]

#[cfg(test)]
mod calibrate_gap_sdk20 {
    use amm_pool_contract::ConstantProductPoolClient;
    use soroban_sdk::Env;

    fn measure_do_expensive_work(env: &Env) {
        let wasm_path = "../target/wasm32v1-none/release/amm_pool_contract.wasm";
        let wasm = std::fs::read(wasm_path).expect("WASM file not found, did you run cargo build?");
        let contract_id = env.register(wasm.as_slice(), ());
        let client = ConstantProductPoolClient::new(env, &contract_id);

        env.mock_all_auths();
        env.budget().reset_unlimited();

        client.do_expensive_work(&10_000);

        let budget = env.budget();
        let cpu = budget.cpu_instruction_cost();
        let mem = budget.memory_bytes_cost();

        println!("=== CALIBRATE_GAP ===");
        println!("CALIBRATE_CPU={}", cpu);
        println!("CALIBRATE_MEM={}", mem);
    }

    #[test]
    fn calibrate_gap() {
        let env = Env::default();
        measure_do_expensive_work(&env);
    }
}
