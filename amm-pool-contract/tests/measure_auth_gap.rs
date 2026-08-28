#![cfg(not(feature = "sdk20"))]

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
    fn measure_auth_gap() {
        let env = Env::default();
        measure_require_auth_only(&env);
    }
}
