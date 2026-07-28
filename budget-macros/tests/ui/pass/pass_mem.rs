//! `#[budget_mem_lt]` shares the expansion with `#[budget_cpu_lt]`, so it gets
//! the same body shapes: unit, trailing expression, and early return.

#[path = "../support/mock_env.rs"]
mod mock_env;

use budget_macros::budget_mem_lt;
use mock_env::{budget_panic, Env};

#[derive(Debug, PartialEq)]
struct TestError;

#[budget_mem_lt(4_096)]
fn unit_body() {
    let env = Env::new(0, 2_048);
}

#[budget_mem_lt(4_096)]
fn result_body() -> Result<u64, TestError> {
    let env = Env::new(0, 2_048);
    Ok(env.cost_estimate().budget().memory_bytes_cost())
}

#[budget_mem_lt(4_096)]
fn early_return_body(exit_early: bool) -> Result<(), TestError> {
    let env = Env::new(0, 8_192);
    if exit_early {
        return Ok(());
    }
    Ok(())
}

fn main() {
    unit_body();
    assert_eq!(result_body(), Ok(2_048));

    let message =
        budget_panic(|| early_return_body(true)).expect("the budget assertion should have failed");
    assert!(
        message.contains("Memory bytes cost 8192 exceeded limit 4096"),
        "unexpected panic message: {message}"
    );
}
