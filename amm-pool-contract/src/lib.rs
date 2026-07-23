#![no_std]
use soroban_sdk::{contract, contractimpl, symbol_short, Bytes, Env, Vec};

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

    /// Writes `n` large byte blobs into temporary storage, exercising
    /// ledger write-bytes budget. Each entry is 256 bytes, so `n = 100`
    /// produces ~25 600 bytes of ledger writes — enough to exceed a tight
    /// write-bytes limit when asserted in tests.
    pub fn do_write_heavy_work(env: Env, n: u32) {
        for i in 0..n {
            // Build a 256-byte payload for each entry so the write footprint
            // grows quickly and is easy to reason about in assertions.
            let mut payload = Bytes::new(&env);
            for _ in 0..256_u32 {
                payload.push_back(i as u8);
            }
            // Use temporary storage so ledger entries are created fresh on
            // every invocation, maximising the measured write bytes.
            env.storage()
                .temporary()
                .set(&(symbol_short!("wh"), i), &payload);
        }
    }
}
