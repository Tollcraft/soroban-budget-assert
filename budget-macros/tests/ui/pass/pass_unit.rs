//! Plain `()` bodies: the assertion runs after the last statement, unchanged
//! from the original macro behavior.
//!
//! Performance note: these fixtures are compiled and run by trybuild, so the
//! only work each body does is the fixed-size mock construction the macro reads
//! back. The accumulator loop in `statement_ends_with_semicolon` was the sole
//! nontrivial operation; it is now a closed-form iterator sum, which keeps the
//! body allocation-free and avoids carrying a mutable accumulator across the
//! injected assertion.

#[path = "../support/mock_env.rs"]
mod mock_env;

use budget_macros::budget_cpu_lt;
use mock_env::{budget_panic, Env};

/// The exact diagnostic an over-limit CPU body must produce.
const EXPECTED_PANIC: &str = "CPU instruction cost 1000 exceeded limit 1000";

#[budget_cpu_lt(1_000)]
fn under_limit() {
    let env = Env::new(999, 0);
    let _ = env.cost_estimate().budget().cpu_instruction_cost();
}

#[budget_cpu_lt(1_000)]
fn zero_cost_under_positive_limit() {
    // Smallest possible cost: 0 < 1000, so the assertion must not panic.
    let env = Env::new(0, 0);
}

#[budget_cpu_lt(1_000)]
fn over_limit() {
    let env = Env::new(1_000, 0);
}

#[budget_cpu_lt(1_000)]
fn statement_ends_with_semicolon() {
    let env = Env::new(500, 0);
    let total: u64 = (0..3).sum();
    assert_eq!(total, 3);
}

fn main() {
    under_limit();
    zero_cost_under_positive_limit();
    statement_ends_with_semicolon();

    let message = budget_panic(over_limit).expect("the budget assertion should have failed");
    assert!(
        message.contains(EXPECTED_PANIC),
        "unexpected panic message: {message}"
    );
}
