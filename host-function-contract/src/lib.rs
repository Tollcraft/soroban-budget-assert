//! # Host-Function Benchmark Fixture (`host-function-contract`)
//!
//! This crate provides a Soroban smart contract fixture designed specifically for
//! empirical cost measurement and baseline gap analysis of repeated host-function invocations.
//!
//! ## Purpose
//!
//! The contract measures repeated calls to `env.ledger().sequence()` without introducing
//! contract storage state, event logging, or CPU-intensive math loops. This isolates the
//! host-function overhead from other billing dimensions (such as read/write bytes or VM instructions).
//!
//! See `README.md` and `MEASUREMENTS.md` at the repository root for detailed methodology
//! and captured figures.

#![no_std]

use soroban_sdk::{contract, contractimpl, Env};

/// Benchmark contract fixture for measuring the gap between local budget estimates
/// and live network simulation figures for repeated host-function calls.
#[contract]
pub struct HostFunctionBenchmark;

#[contractimpl]
impl HostFunctionBenchmark {
    /// Calls the `env.ledger().sequence()` host function repeatedly for `iterations` count
    /// and returns the final sequence value.
    ///
    /// Returning the sequence value prevents the compiler from optimizing out the loop
    /// while keeping the execution entirely free of storage side-effects.
    ///
    /// # Arguments
    ///
    /// * `env` - The Soroban environment instance.
    /// * `iterations` - Number of times to invoke `env.ledger().sequence()`.
    pub fn repeated_sequence(env: Env, iterations: u32) -> u32 {
        let mut sequence = 0;

        for _ in 0..iterations {
            sequence = env.ledger().sequence();
        }

        sequence
    }
}
