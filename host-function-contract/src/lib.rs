#![no_std]

use soroban_sdk::{contract, contractimpl, Env};

/// Benchmark fixture for measuring the gap between local and network costs
/// for repeated host-function calls.
#[contract]
pub struct HostFunctionBenchmark;

#[contractimpl]
impl HostFunctionBenchmark {
    /// Calls the ledger sequence host function repeatedly and returns the
    /// final value. The return value prevents the loop from being eliminated
    /// while keeping the benchmark independent of contract storage.
    pub fn repeated_sequence(env: Env, iterations: u32) -> u32 {
        let mut sequence = 0;

        for _ in 0..iterations {
            sequence = env.ledger().sequence();
        }

        sequence
    }
}
