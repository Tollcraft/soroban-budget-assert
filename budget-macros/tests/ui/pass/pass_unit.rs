//! Plain `()` bodies: the assertion runs after the last statement, unchanged
//! from the original macro behavior.

#[path = "../support/mock_env.rs"]
mod mock_env;

use budget_macros::budget_cpu_lt;
use mock_env::{budget_panic, Env};

#[budget_cpu_lt(1_000)]
fn under_limit() {
    let env = Env::new(999, 0);
    let _ = env.cost_estimate().budget().cpu_instruction_cost();
}

#[budget_cpu_lt(1_000)]
fn over_limit() {
    let env = Env::new(1_000, 0);
}

#[budget_cpu_lt(1_000)]
fn statement_ends_with_semicolon() {
    let env = Env::new(500, 0);
    let mut total = 0;
    for i in 0..3 {
        total += i;
    }
    assert_eq!(total, 3);
}

fn main() {
    under_limit();
    statement_ends_with_semicolon();

    let message = budget_panic(over_limit).expect("the budget assertion should have failed");
    assert!(
        message.contains("CPU instruction cost 1000 exceeded limit 1000"),
        "unexpected panic message: {message}"
    );
}
