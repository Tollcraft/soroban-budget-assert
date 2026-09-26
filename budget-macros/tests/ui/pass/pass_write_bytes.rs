//! `#[budget_write_bytes_lt]` is instrumented by the same shared helper, so it
//! supports the same body shapes: unit, trailing expression, and early return.
//! It reports `memory_bytes_cost()` as its write-bytes proxy.
//!
//! The fixture is split into focused pass/failure helpers so each body shape,
//! the strict limit boundary, and metric isolation can be reviewed and run
//! independently.

#[path = "../support/mock_env.rs"]
mod mock_env;

use budget_macros::budget_write_bytes_lt;
use mock_env::{budget_panic, Env};

const EXPECTED_COST: u64 = 2_048;

#[derive(Debug, PartialEq)]
struct TestError;

#[budget_write_bytes_lt(4_096)]
fn unit_body() {
    let env = Env::new(0, EXPECTED_COST);
}

#[budget_write_bytes_lt(4_096)]
fn result_body() -> Result<u64, TestError> {
    let env = Env::new(0, EXPECTED_COST);
    Ok(env.cost_estimate().budget().memory_bytes_cost())
}

#[budget_write_bytes_lt(4_096)]
fn early_return_under_limit(exit_early: bool) -> Result<(), TestError> {
    let env = Env::new(0, EXPECTED_COST);
    if exit_early {
        return Ok(());
    }
    Ok(())
}

#[budget_write_bytes_lt(4_096)]
fn early_return_body(exit_early: bool) -> Result<(), TestError> {
    let env = Env::new(0, 8_192);
    if exit_early {
        return Ok(());
    }
    Ok(())
}

#[budget_write_bytes_lt(4_096)]
fn just_below_limit() {
    let env = Env::new(0, 4_095);
}

#[budget_write_bytes_lt(4_096)]
fn exact_limit_body() -> Result<(), TestError> {
    let env = Env::new(0, 4_096);
    Ok(())
}

#[budget_write_bytes_lt(1)]
fn cpu_cost_does_not_use_memory_proxy() {
    let env = Env::new(u64::MAX, 0);
}

fn assert_budget_failure<F, R>(case: &str, expected: &str, f: F)
where
    F: FnOnce() -> R + std::panic::UnwindSafe,
{
    let message = budget_panic(f)
        .unwrap_or_else(|| panic!("{case}: the budget assertion should have failed"));
    assert!(
        message.contains(expected),
        "{case}: unexpected panic message: {message}"
    );
}

fn run_passing_cases() {
    unit_body();
    assert_eq!(result_body(), Ok(EXPECTED_COST));
    assert_eq!(early_return_under_limit(true), Ok(()));
    assert_eq!(early_return_under_limit(false), Ok(()));
    just_below_limit();
    cpu_cost_does_not_use_memory_proxy();
}

fn run_failing_cases() {
    assert_budget_failure(
        "strict boundary",
        "Write bytes cost (memory proxy) 4096 exceeded limit 4096",
        exact_limit_body,
    );
    assert_budget_failure(
        "early return",
        "Write bytes cost (memory proxy) 8192 exceeded limit 4096",
        || early_return_body(true),
    );
    assert_budget_failure(
        "fall-through return body",
        "Write bytes cost (memory proxy) 8192 exceeded limit 4096",
        || early_return_body(false),
    );
}

fn main() {
    run_passing_cases();
    run_failing_cases();
}
