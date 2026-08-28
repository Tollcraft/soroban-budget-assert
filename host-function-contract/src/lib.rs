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
//! It also hosts the Map-operation fixtures (`map_insert`, `map_get`, `map_remove`,
//! `map_iterate`), which isolate Soroban [`Map`] host calls without storage, event, or
//! arithmetic side-effects, for the local-vs-network Map cost-gap measurement.
//!
//! See `README.md` and `MEASUREMENTS.md` at the repository root for detailed methodology
//! and captured figures.

#![no_std]

use soroban_sdk::{contract, contractimpl, Bytes, Env, Map};

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

    /// Calls `env.ledger().timestamp()` repeatedly for `iterations` count
    /// and returns the final timestamp value.
    ///
    /// This isolates the cost of a different ledger-read host function from
    /// `repeated_sequence` to test whether the local-vs-network gap varies
    /// across distinct host functions within the same module.
    pub fn repeated_timestamp(env: Env, iterations: u32) -> u64 {
        let mut timestamp: u64 = 0;

        for _ in 0..iterations {
            timestamp = env.ledger().timestamp();
        }

        timestamp
    }

    /// Hashes a small input buffer with SHA-256 for `iterations` count and
    /// returns the number of iterations completed.
    ///
    /// Each iteration allocates an 8-byte `Bytes` value and passes it through
    /// `env.crypto().sha256()`, exercising the cryptographic host function
    /// category. The return value prevents dead-code elimination while
    /// keeping the function free of storage side-effects.
    pub fn repeated_hash(env: Env, iterations: u32) -> u32 {
        let input = Bytes::from_slice(&env, b"hashben");

        for _ in 0..iterations {
            let _digest = env.crypto().sha256(&input);
        }

        iterations
    }

    /// Creates `iterations` fresh `Bytes` values via `Bytes::new(&env)` and
    /// returns the count completed.
    ///
    /// Each iteration exercises the Bytes-allocation host function without
    /// any storage, event, or cryptographic side-effects, isolating the
    /// per-call cost of the Bytes constructor.
    pub fn repeated_bytes_new(env: Env, iterations: u32) -> u32 {
        for _ in 0..iterations {
            let _b = Bytes::new(&env);
        }

        iterations
    }

    /// Inserts `size` key/value pairs into a fresh `Map<u32, u32>` and returns
    /// the number of inserts performed.
    ///
    /// Every operation is a Map insert host call; no storage, event, or
    /// arithmetic side-effects. The return value prevents dead-code
    /// elimination. `size` is the map size / insert count.
    pub fn map_insert(env: Env, size: u32) -> u32 {
        let mut m: Map<u32, u32> = Map::new(&env);

        for i in 0..size {
            m.set(i, i);
        }

        size
    }

    /// Builds a `size`-entry `Map<u32, u32>` and then issues `size` `get`
    /// calls against the existing keys, returning the number of gets.
    ///
    /// The build (insert) cost is identical to that of [`Self::map_insert`] at
    /// the same `size`, so the marginal cost of the get loop is obtained by
    /// subtracting `map_insert(size)`. This isolates the cost of Map lookups.
    pub fn map_get(env: Env, size: u32) -> u32 {
        let mut m: Map<u32, u32> = Map::new(&env);
        for i in 0..size {
            m.set(i, i);
        }

        for i in 0..size {
            let _v = m.get(i);
        }

        size
    }

    /// Builds a `size`-entry `Map<u32, u32>` and then issues `size` `remove`
    /// calls, deleting every key, returning the number of removes.
    ///
    /// The build (insert) cost is identical to that of [`Self::map_insert`] at
    /// the same `size`, so the marginal cost of the remove loop is obtained by
    /// subtracting `map_insert(size)`. This isolates the cost of Map removals.
    pub fn map_remove(env: Env, size: u32) -> u32 {
        let mut m: Map<u32, u32> = Map::new(&env);
        for i in 0..size {
            m.set(i, i);
        }

        for i in 0..size {
            let _ = m.remove(i);
        }

        size
    }

    /// Builds a `size`-entry `Map<u32, u32>` and then iterates over every
    /// entry, summing the values and returning the sum.
    ///
    /// The build (insert) cost is identical to that of [`Self::map_insert`] at
    /// the same `size`, so the marginal cost of the iteration is obtained by
    /// subtracting `map_insert(size)`. This isolates the cost of Map
    /// iteration.
    pub fn map_iterate(env: Env, size: u32) -> u32 {
        let mut m: Map<u32, u32> = Map::new(&env);
        for i in 0..size {
            m.set(i, i);
        }

        let mut sum: u32 = 0;
        for (_k, v) in m.iter() {
            sum = sum.wrapping_add(v);
        }

        sum
    }
}
