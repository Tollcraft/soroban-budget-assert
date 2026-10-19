//! Constant-product AMM helpers extracted from the pool contract.
//!
//! These are the calculations and storage mutations that previously sat inline
//! in `deposit` / `swap` / `withdraw`. Moving them here keeps the
//! `#[contractimpl]` entrypoints as thin orchestration and makes the maths
//! testable on its own.
//!
//! Behaviour is preserved exactly as it was before the extraction, including
//! the integer-truncation and `min`-ratio choices noted below.

use soroban_sdk::Env;

use crate::{Error, BAL_A, BAL_B, LP_BAL, RESERVE_A, RESERVE_B, TOTAL_SHARES};

/// Seeds the reserve and share keys at zero.
///
/// Called from `initialize` only after its already-initialized guard passes,
/// so a re-initialize can never clobber live reserves.
pub fn initialize_storage(env: &Env) {
    env.storage().instance().set(&RESERVE_A, &0i128);
    env.storage().instance().set(&RESERVE_B, &0i128);
    env.storage().instance().set(&TOTAL_SHARES, &0i128);
}

/// Reads both reserves plus the outstanding LP share count.
///
/// `initialize` sets all three keys before any deposit, swap, or withdraw, so
/// a missing key here is a programming error rather than a user-facing one.
pub fn get_reserves(env: &Env) -> (i128, i128, i128) {
    (
        env.storage().instance().get(&RESERVE_A).unwrap(),
        env.storage().instance().get(&RESERVE_B).unwrap(),
        env.storage().instance().get(&TOTAL_SHARES).unwrap(),
    )
}

/// Reads just the two reserves, for the swap path which never touches shares.
pub fn get_reserves_pair(env: &Env) -> (i128, i128) {
    (
        env.storage().instance().get(&RESERVE_A).unwrap(),
        env.storage().instance().get(&RESERVE_B).unwrap(),
    )
}

/// LP shares minted for a deposit of `amount_a` / `amount_b`.
///
/// The first deposit (`total_shares == 0`) mints the geometric mean of the two
/// amounts — the standard Uniswap v2 convention, `sqrt(amount_a * amount_b)`,
/// with `isqrt` giving the integer floor square root.
///
/// Later deposits mint proportionally and take the *smaller* of the two ratios,
/// so a skewed deposit cannot dilute existing LPs.
pub fn calculate_shares(
    amount_a: i128,
    amount_b: i128,
    total_shares: i128,
    reserve_a: i128,
    reserve_b: i128,
) -> i128 {
    if total_shares == 0 {
        (amount_a * amount_b).isqrt()
    } else {
        let from_a = amount_a * total_shares / reserve_a;
        let from_b = amount_b * total_shares / reserve_b;
        from_a.min(from_b)
    }
}

/// Constant-product output for `amount_in`, i.e.
/// `out_reserve * amount_in / (in_reserve + amount_in)`.
///
/// Division truncates toward zero, which slightly favours the pool.
pub fn calculate_amount_out(out_reserve: i128, amount_in: i128, in_reserve: i128) -> i128 {
    out_reserve * amount_in / (in_reserve + amount_in)
}

/// Fails when a swap would return less than the caller's minimum.
pub fn check_slippage(amount_out: i128, min_amount_out: i128) -> Result<(), Error> {
    if amount_out < min_amount_out {
        return Err(Error::SlippageExceeded);
    }
    Ok(())
}

/// Pro-rata amounts returned for burning `shares`.
pub fn calculate_withdraw_amounts(
    reserve_a: i128,
    reserve_b: i128,
    shares: i128,
    total_shares: i128,
) -> (i128, i128) {
    (
        reserve_a * shares / total_shares,
        reserve_b * shares / total_shares,
    )
}

/// Fails when a withdrawal would return less than either minimum.
pub fn check_slippage_withdraw(
    amount_a: i128,
    min_a: i128,
    amount_b: i128,
    min_b: i128,
) -> Result<(), Error> {
    if amount_a < min_a || amount_b < min_b {
        return Err(Error::SlippageExceeded);
    }
    Ok(())
}

/// Applies a deposit (`is_deposit == true`) or a withdrawal to the tracked
/// balances, the reserves, the share count, and the LP balance.
///
/// Both directions differ only in sign, which is why they share one helper
/// instead of two near-identical blocks.
pub fn update_balances_and_reserves(
    env: &Env,
    amount_a: i128,
    amount_b: i128,
    shares: i128,
    is_deposit: bool,
) {
    let sign: i128 = if is_deposit { 1 } else { -1 };

    let bal_a: i128 = env.storage().instance().get(&BAL_A).unwrap_or(0);
    let bal_b: i128 = env.storage().instance().get(&BAL_B).unwrap_or(0);
    let reserve_a: i128 = env.storage().instance().get(&RESERVE_A).unwrap();
    let reserve_b: i128 = env.storage().instance().get(&RESERVE_B).unwrap();
    let total_shares: i128 = env.storage().instance().get(&TOTAL_SHARES).unwrap();
    let lp_bal: i128 = env.storage().instance().get(&LP_BAL).unwrap_or(0);

    env.storage()
        .instance()
        .set(&BAL_A, &(bal_a + sign * amount_a));
    env.storage()
        .instance()
        .set(&BAL_B, &(bal_b + sign * amount_b));
    env.storage()
        .instance()
        .set(&RESERVE_A, &(reserve_a + sign * amount_a));
    env.storage()
        .instance()
        .set(&RESERVE_B, &(reserve_b + sign * amount_b));
    env.storage()
        .instance()
        .set(&TOTAL_SHARES, &(total_shares + sign * shares));
    env.storage()
        .instance()
        .set(&LP_BAL, &(lp_bal + sign * shares));
}

/// Moves the reserves and tracked balances after a swap.
///
/// When `is_a_in` the A side is the input and the B side is the output; the
/// pattern reverses otherwise. The tracked balances (`BAL_A` / `BAL_B`) are
/// simulated holdings inside this benchmarking fixture, not on-chain token
/// transfers — the reserves drive pricing, the balances record what a caller
/// is owed.
pub fn update_balances_and_reserves_after_swap(
    env: &Env,
    is_a_in: bool,
    amount_in: i128,
    amount_out: i128,
) {
    let bal_a: i128 = env.storage().instance().get(&BAL_A).unwrap_or(0);
    let bal_b: i128 = env.storage().instance().get(&BAL_B).unwrap_or(0);
    let reserve_a: i128 = env.storage().instance().get(&RESERVE_A).unwrap();
    let reserve_b: i128 = env.storage().instance().get(&RESERVE_B).unwrap();

    let in_a: i128 = if is_a_in { amount_in } else { 0 };
    let out_a: i128 = if is_a_in { 0 } else { amount_out };
    let in_b: i128 = if is_a_in { 0 } else { amount_in };
    let out_b: i128 = if is_a_in { amount_out } else { 0 };

    env.storage()
        .instance()
        .set(&BAL_A, &(bal_a + in_a - out_a));
    env.storage()
        .instance()
        .set(&BAL_B, &(bal_b + in_b - out_b));
    env.storage()
        .instance()
        .set(&RESERVE_A, &(reserve_a + in_a - out_a));
    env.storage()
        .instance()
        .set(&RESERVE_B, &(reserve_b + in_b - out_b));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_shares_first_deposit_geometric_mean_floor_isqrt() {
        // first deposit (total_shares == 0) mints floor-of-isqrt of amount_a * amount_b
        let shares = calculate_shares(100, 400, 0, 0, 0);
        assert_eq!(shares, 200);

        // Non-exact square root tests floor truncation
        let shares_trunc = calculate_shares(10, 10, 0, 0, 0);
        assert_eq!(shares_trunc, 10);

        let shares_non_exact = calculate_shares(10, 15, 0, 0, 0);
        // sqrt(150) = 12.247 -> 12
        assert_eq!(shares_non_exact, 12);
    }

    #[test]
    fn test_calculate_shares_skewed_deposit_min_ratio() {
        // Later deposits mint the smaller of the two proportional ratios
        // reserve_a = 1000, reserve_b = 2000, total_shares = 500
        // from_a = 200 * 500 / 1000 = 100
        // from_b = 600 * 500 / 2000 = 150
        let shares = calculate_shares(200, 600, 500, 1000, 2000);
        assert_eq!(shares, 100);

        // Skewed the other way: from_b is smaller
        // from_a = 400 * 500 / 1000 = 200
        // from_b = 200 * 500 / 2000 = 50
        let shares_b_smaller = calculate_shares(400, 200, 500, 1000, 2000);
        assert_eq!(shares_b_smaller, 50);
    }

    #[test]
    fn type_test_calculate_shares_zero_amounts() {
        let shares = calculate_shares(0, 0, 0, 0, 0);
        assert_eq!(shares, 0);
    }

    #[test]
    fn test_calculate_amount_out_fee_application_and_truncation() {
        // out_reserve = 1000, amount_in = 100, in_reserve = 1000
        // formula: out_reserve * amount_in / (in_reserve + amount_in)
        // 1000 * 100 / (1000 + 100) = 100000 / 1100 = 90.9090...
        // truncation toward zero gives 90
        let out = calculate_amount_out(1000, 100, 1000);
        assert_eq!(out, 90);

        let out_zero = calculate_amount_out(1000, 0, 1000);
        assert_eq!(out_zero, 0);
    }

    #[test]
    fn test_check_slippage_passing_and_failing() {
        assert!(check_slippage(100, 90).is_ok());
        assert!(check_slippage(100, 100).is_ok());

        let err = check_slippage(99, 100);
        assert_eq!(err, Err(Error::SlippageExceeded));
    }

    #[test]
    fn test_calculate_withdraw_amounts_pro_rata() {
        // reserve_a = 1000, reserve_b = 2000, shares = 100, total_shares = 1000
        // amount_a = 1000 * 100 / 1000 = 100
        // amount_b = 2000 * 100 / 1000 = 200
        let (a, b) = calculate_withdraw_amounts(1000, 2000, 100, 1000);
        assert_eq!(a, 100);
        assert_eq!(b, 200);
    }

    #[test]
    fn test_check_slippage_withdraw_passing_and_failing() {
        assert!(check_slippage_withdraw(100, 90, 200, 190).is_ok());
        assert!(check_slippage_withdraw(100, 100, 200, 200).is_ok());

        // Fails when a is below min_a
        assert_eq!(
            check_slippage_withdraw(99, 100, 200, 190),
            Err(Error::SlippageExceeded)
        );

        // Fails when b is below min_b
        assert_eq!(
            check_slippage_withdraw(100, 90, 189, 190),
            Err(Error::SlippageExceeded)
        );
    }
}
