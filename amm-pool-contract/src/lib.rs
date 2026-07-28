#![no_std]
use soroban_sdk::{
    contract, contractimpl, symbol_short, vec, Address, Bytes, Env, Symbol, Val, Vec,
};

const RESERVE_A: Symbol = symbol_short!("resA");
const RESERVE_B: Symbol = symbol_short!("resB");
const TOTAL_SHARES: Symbol = symbol_short!("shares");
const BAL_A: Symbol = symbol_short!("balA");
const BAL_B: Symbol = symbol_short!("balB");
const LP_BAL: Symbol = symbol_short!("lpBl");

#[contract]
pub struct HelperContract;

#[contractimpl]
impl HelperContract {
    /// Multiplies two `u32` values using wrapping (modular) arithmetic.
    ///
    /// `wrapping_mul` is chosen deliberately for the cross-contract
    /// benchmark: on overflow it wraps around instead of panicking in
    /// debug builds, so large inputs won't crash the test and obscure
    /// the cost measurement.  The result is meaningless for real use —
    /// this contract exists purely as a cross-contract call target for
    /// budget measurement.
    pub fn multiply(_env: Env, a: u32, b: u32) -> u32 {
        a.wrapping_mul(b)
    }
}

#[contract]
pub struct ConstantProductPool;

#[contractimpl]
impl ConstantProductPool {
    pub fn initialize(env: Env) {
        if env.storage().instance().has(&RESERVE_A) {
            panic!("already initialized");
        }
        env.storage().instance().set(&RESERVE_A, &0i128);
        env.storage().instance().set(&RESERVE_B, &0i128);
        env.storage().instance().set(&TOTAL_SHARES, &0i128);
    }

    pub fn deposit(env: Env, to: Address, amount_a: i128, amount_b: i128) -> i128 {
        to.require_auth();

        let reserve_a: i128 = env.storage().instance().get(&RESERVE_A).unwrap();
        let reserve_b: i128 = env.storage().instance().get(&RESERVE_B).unwrap();
        let total_shares: i128 = env.storage().instance().get(&TOTAL_SHARES).unwrap();

        // ── LP share calculation ───────────────────────────────────────
        //
        // If this is the first deposit (total_shares == 0), the initial
        // LP shares are set to the geometric mean of the two token amounts.
        // `isqrt()` computes the integer (floor) square root — this is the
        // standard Uniswap v2 / constant-product AMM convention for
        // initial share minting:  shares = sqrt(amount_a * amount_b).
        //
        // For subsequent deposits, shares are minted proportionally — the
        // depositor receives whichever of the two token ratios yields the
        // smaller share count (protecting existing LPs from dilution).
        let shares = if total_shares == 0 {
            (amount_a * amount_b).isqrt()
        } else {
            let from_a = amount_a * total_shares / reserve_a;
            let from_b = amount_b * total_shares / reserve_b;
            from_a.min(from_b)
        };

        let bal_a: i128 = env.storage().instance().get(&BAL_A).unwrap_or(0);
        let bal_b: i128 = env.storage().instance().get(&BAL_B).unwrap_or(0);
        let lp_bal: i128 = env.storage().instance().get(&LP_BAL).unwrap_or(0);

        env.storage().instance().set(&BAL_A, &(bal_a + amount_a));
        env.storage().instance().set(&BAL_B, &(bal_b + amount_b));
        env.storage()
            .instance()
            .set(&RESERVE_A, &(reserve_a + amount_a));
        env.storage()
            .instance()
            .set(&RESERVE_B, &(reserve_b + amount_b));
        env.storage()
            .instance()
            .set(&TOTAL_SHARES, &(total_shares + shares));
        env.storage().instance().set(&LP_BAL, &(lp_bal + shares));

        env.events()
            .publish(("deposit",), (to, amount_a, amount_b, shares));

        shares
    }

    pub fn swap(
        env: Env,
        to: Address,
        is_a_in: bool,
        amount_in: i128,
        min_amount_out: i128,
    ) -> i128 {
        to.require_auth();

        let reserve_a: i128 = env.storage().instance().get(&RESERVE_A).unwrap();
        let reserve_b: i128 = env.storage().instance().get(&RESERVE_B).unwrap();

        let (in_reserve, out_reserve) = if is_a_in {
            (reserve_a, reserve_b)
        } else {
            (reserve_b, reserve_a)
        };

        // ── Constant-product swap formula ──────────────────────────────
        //
        // Uses the classic constant-product AMM invariant x·y = k
        // (popularised by Uniswap v2).
        //
        //   Before:  in_reserve · out_reserve = k
        //   After:   (in_reserve + amount_in) · (out_reserve - amount_out) = k
        //
        // Solving for `amount_out` (with no swap fee — this is a
        // benchmarking fixture, not a production pool):
        //
        //   amount_out = (out_reserve · amount_in) / (in_reserve + amount_in)
        //
        // Soroban uses integer arithmetic (i128); division truncates
        // toward zero, which slightly favours the pool.
        let amount_out = out_reserve * amount_in / (in_reserve + amount_in);

        if amount_out < min_amount_out {
            panic!("slippage exceeded");
        }

        let bal_a: i128 = env.storage().instance().get(&BAL_A).unwrap_or(0);
        let bal_b: i128 = env.storage().instance().get(&BAL_B).unwrap_or(0);

        // ── Update per-user and pool balances after swap ────────────────
        //
        // The conditionals below encode which side is the "in" asset (added
        // to the pool / deducted from the user's tracked balance) vs the
        // "out" asset (removed from the pool / added to the user's tracked
        // balance).  When `is_a_in` is true:
        //   • A flows into the pool   → bal_a decreases, reserve_a increases
        //   • B flows out of the pool → bal_b increases, reserve_b decreases
        // The pattern reverses when `is_a_in` is false.
        //
        // These tracked balances (`BAL_A`, `BAL_B`) are simulated token
        // holdings within the contract — they are not on-chain token
        // transfers.  The reserves drive the pricing formula; the balances
        // merely track what the user is owed, as an approximate substitute
        // for real token contracts in this benchmarking fixture.
        let new_bal_a =
            bal_a + if is_a_in { amount_in } else { 0 } - if is_a_in { 0 } else { amount_out };
        let new_bal_b =
            bal_b + if is_a_in { 0 } else { amount_in } - if is_a_in { amount_out } else { 0 };
        let new_reserve_a =
            reserve_a + if is_a_in { amount_in } else { 0 } - if is_a_in { 0 } else { amount_out };
        let new_reserve_b =
            reserve_b + if is_a_in { 0 } else { amount_in } - if is_a_in { amount_out } else { 0 };

        env.storage().instance().set(&BAL_A, &new_bal_a);
        env.storage().instance().set(&BAL_B, &new_bal_b);
        env.storage().instance().set(&RESERVE_A, &new_reserve_a);
        env.storage().instance().set(&RESERVE_B, &new_reserve_b);

        env.events()
            .publish(("swap",), (to, is_a_in, amount_in, amount_out));

        amount_out
    }

    pub fn withdraw(env: Env, to: Address, shares: i128, min_a: i128, min_b: i128) -> (i128, i128) {
        to.require_auth();

        let reserve_a: i128 = env.storage().instance().get(&RESERVE_A).unwrap();
        let reserve_b: i128 = env.storage().instance().get(&RESERVE_B).unwrap();
        let total_shares: i128 = env.storage().instance().get(&TOTAL_SHARES).unwrap();

        let amount_a = reserve_a * shares / total_shares;
        let amount_b = reserve_b * shares / total_shares;

        if amount_a < min_a || amount_b < min_b {
            panic!("slippage exceeded");
        }

        let bal_a: i128 = env.storage().instance().get(&BAL_A).unwrap_or(0);
        let bal_b: i128 = env.storage().instance().get(&BAL_B).unwrap_or(0);
        let lp_bal: i128 = env.storage().instance().get(&LP_BAL).unwrap_or(0);

        env.storage().instance().set(&BAL_A, &(bal_a - amount_a));
        env.storage().instance().set(&BAL_B, &(bal_b - amount_b));
        env.storage()
            .instance()
            .set(&RESERVE_A, &(reserve_a - amount_a));
        env.storage()
            .instance()
            .set(&RESERVE_B, &(reserve_b - amount_b));
        env.storage()
            .instance()
            .set(&TOTAL_SHARES, &(total_shares - shares));
        env.storage().instance().set(&LP_BAL, &(lp_bal - shares));

        env.events()
            .publish(("withdraw",), (to, shares, amount_a, amount_b));

        (amount_a, amount_b)
    }

    pub fn require_auth_only(_env: Env, addr: Address) {
        addr.require_auth();
    }

    /// Extends the TTL of this contract's instance storage (and its Wasm
    /// code) so neither is evicted from the ledger before `extend_to`
    /// ledgers from now, provided the current TTL is below `threshold`
    /// ledgers. Touches no pool state; isolated purely so its cost can be
    /// measured/asserted on its own, the same way `require_auth_only`
    /// isolates `require_auth`.
    pub fn extend_instance_ttl(env: Env, threshold: u32, extend_to: u32) {
        env.storage().instance().extend_ttl(threshold, extend_to);
    }

    pub fn do_expensive_work(env: Env, n: u32) -> u32 {
        let mut result: u32 = 0;

        // ── Arithmetic-heavy CPU benchmark loop ────────────────────────
        // Accumulates the sum of i² for i in 0..n using wrapping
        // arithmetic.  Both `wrapping_mul` (i·i) and `wrapping_add`
        // (result + i²) are used so the loop cannot panic on overflow in
        // debug builds — panicking would terminate the test early and
        // report a misleadingly low cost estimate.
        //
        // The result value is discarded; this loop exists purely to
        // consume CPU instructions for budget measurement.
        for i in 0..n {
            result = result.wrapping_add(i.wrapping_mul(i));
        }

        let mut vec = Vec::new(&env);
        for i in 0..(n.min(100)) {
            vec.push_back(i);
        }
        env.storage().instance().set(&symbol_short!("vec"), &vec);

        result
    }

    pub fn burn_resources(env: Env, n: u32) -> u32 {
        let mut acc: u32 = 0;

        for i in 0..n {
            acc = acc.wrapping_add(i.wrapping_mul(i).wrapping_add(1));
            for j in 0..n.min(100) {
                acc = acc.wrapping_add(j.wrapping_mul(j));
            }
        }

        let mut vec = Vec::new(&env);
        for i in 0..n.min(200) {
            vec.push_back(i);
        }
        env.storage().instance().set(&symbol_short!("burn"), &vec);

        let mut vec2 = Vec::new(&env);
        for i in 0..n.min(200) {
            vec2.push_back(i.wrapping_mul(i));
        }
        env.storage().instance().set(&symbol_short!("brn2"), &vec2);

        acc
    pub fn do_cross_contract_work(env: Env, other: Address, n: u32) -> u32 {
        let mut result: u32 = 0;
        // ── Cross-contract CPU benchmark loop ─────────────────────────
        // Invokes `HelperContract::multiply` n times, accumulating the
        // returned products with `wrapping_add` to avoid debug-mode
        // panics on overflow.  The cross-contract call overhead (host
        // function dispatch, WASM boundary crossing) exercises a
        // different cost profile than the pure-arithmetic loop in
        // `do_expensive_work`, making this a complementary benchmark.
        for i in 0..n {
            let product: u32 = env.invoke_contract(
                &other,
                &symbol_short!("multiply"),
                vec![&env, Val::from(i), Val::from(i)],
            );
            result = result.wrapping_add(product);
        }
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

    /// Reads `n` keys from instance storage in a loop, exercising the
    /// ledger read-bytes budget. Writes each key once upfront so the reads
    /// hit persisted data, then reads them all back to accumulate
    /// `disk_read_bytes`.
    ///
    /// Each key stores a 256-byte `Bytes` value so the read footprint
    /// grows quickly and is easy to reason about in assertions.
    /// `n = 100` produces ~25 600 bytes of ledger reads from the read
    /// phase alone.
    /// Allocates `n` host-resident `Vec<u32>` elements with no storage or
    /// authorization side-effects, isolating the memory-bytes cost of a
    /// pure allocation loop.
    ///
    /// Returned as `u32` so the assertion has a deterministic cross-check
    /// (`v.len()` versus `n`). The fixture exists for the
    /// `local-vs-network memory-bytes gap` measurement series (issue
    /// #122); by exercising only `Vec::new(&env).push_back` it minimises
    /// the write/storage/auth cost surface so the simulation's reported
    /// `result.cost.memBytes` is dominated by allocation.
    pub fn allocate_vec(env: Env, n: u32) -> u32 {
        let mut v: Vec<u32> = Vec::new(&env);
        for i in 0..n {
            v.push_back(i);
        }
        let len = v.len();
        // Drop the Vec explicitly so a future change to in-host GC cost
        // doesn't silently slip into the measurement.
        drop(v);
        len
    }

    pub fn do_read_heavy_work(env: Env, n: u32) -> u32 {
        // Phase 1: Write n keys to instance storage so they can be read back.
        for i in 0..n {
            let mut payload = Bytes::new(&env);
            for _ in 0..256_u32 {
                payload.push_back(i as u8);
            }
            env.storage()
                .instance()
                .set(&(symbol_short!("rh"), i), &payload);
        }

        // Phase 2: Read all keys back to accumulate disk_read_bytes.
        // The `sum` return value prevents the compiler from optimizing
        // away the reads and gives us a simple correctness check.
        let mut sum: u32 = 0;
        for i in 0..n {
            let val: Bytes = env
                .storage()
                .instance()
                .get(&(symbol_short!("rh"), i))
                .unwrap_or_else(|| Bytes::new(&env));
            sum = sum.wrapping_add(val.len());
        }
        sum
    }
}
