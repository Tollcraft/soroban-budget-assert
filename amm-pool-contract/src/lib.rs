#![no_std]
use soroban_sdk::{contract, contractimpl, symbol_short, Address, Env, Symbol, Vec};

const RESERVE_A: Symbol = symbol_short!("resA");
const RESERVE_B: Symbol = symbol_short!("resB");
const TOTAL_SHARES: Symbol = symbol_short!("shares");
const BAL_A: Symbol = symbol_short!("balA");
const BAL_B: Symbol = symbol_short!("balB");
const LP_BAL: Symbol = symbol_short!("lpBl");

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

        let amount_out = out_reserve * amount_in / (in_reserve + amount_in);

        if amount_out < min_amount_out {
            panic!("slippage exceeded");
        }

        let bal_a: i128 = env.storage().instance().get(&BAL_A).unwrap_or(0);
        let bal_b: i128 = env.storage().instance().get(&BAL_B).unwrap_or(0);

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

    pub fn do_expensive_work(env: Env, n: u32) -> u32 {
        let mut result: u32 = 0;

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
}
