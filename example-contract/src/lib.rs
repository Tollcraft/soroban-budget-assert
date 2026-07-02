#![no_std]
use soroban_sdk::{contract, contractimpl, Env, Vec, symbol_short};

#[contract]
pub struct ExpensiveContract;

#[contractimpl]
impl ExpensiveContract {
    pub fn do_expensive_work(env: Env, n: u32) -> u32 {
        let mut result: u32 = 0;
        
        // Compute expensive
        for i in 0..n {
            result = result.wrapping_add(i.wrapping_mul(i));
        }

        // Storage expensive
        let mut vec = Vec::new(&env);
        for i in 0..(n.min(100)) {
            vec.push_back(i);
        }
        env.storage().instance().set(&symbol_short!("vec"), &vec);

        result
    }
}
