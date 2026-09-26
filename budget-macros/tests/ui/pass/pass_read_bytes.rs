//! `#[budget_read_bytes_lt]` is instrumented by the same shared helper, so it
//! supports the same body shapes: unit, trailing expression, early return, and
//! `Result` errors. It reports `memory_bytes_cost()` as its read-bytes proxy.
//!
//! Baseline, environment-sourced, and percentage limits have dedicated fixtures;
//! this file covers the direct-limit behavior, strict boundaries, metric
//! isolation, the full `u64` range, and application to an `impl` block.

#[path = "../support/mock_env.rs"]
mod mock_env;

use budget_macros::budget_read_bytes_lt;
use mock_env::{budget_panic, Env};

const EXPECTED_COST: u64 = 2_048;

#[derive(Debug, PartialEq)]
struct TestError;

struct Contract;

#[budget_read_bytes_lt(4_096)]
fn unit_body() {
    let env = Env::new(0, EXPECTED_COST);
}

#[budget_read_bytes_lt(4_096)]
fn result_body() -> Result<u64, TestError> {
    let env = Env::new(0, EXPECTED_COST);
    Ok(env.cost_estimate().budget().memory_bytes_cost())
}

#[budget_read_bytes_lt(4_096)]
fn result_error_body() -> Result<(), TestError> {
    let env = Env::new(0, EXPECTED_COST);
    Err(TestError)
}

#[budget_read_bytes_lt(4_096)]
fn early_return_under_limit(exit_early: bool) -> Result<(), TestError> {
    let env = Env::new(0, EXPECTED_COST);
    if exit_early {
        return Ok(());
    }
    Ok(())
}

#[budget_read_bytes_lt(4_096)]
fn early_return_body(exit_early: bool) -> Result<(), TestError> {
    let env = Env::new(0, 8_192);
    if exit_early {
        return Ok(());
    }
    Ok(())
}

#[budget_read_bytes_lt(4_096)]
fn just_below_limit() {
    let env = Env::new(0, 4_095);
}

#[budget_read_bytes_lt(1)]
fn zero_memory_is_below_positive_limit() {
    let env = Env::new(0, 0);
}

#[budget_read_bytes_lt(1)]
fn cpu_cost_does_not_use_memory_proxy() {
    let env = Env::new(u64::MAX, 0);
}

#[budget_read_bytes_lt(18_446_744_073_709_551_615)]
fn largest_cost_below_u64_max() {
    let env = Env::new(0, u64::MAX - 1);
}

#[budget_read_bytes_lt(18_446_744_073_709_551_615)]
fn largest_cost_at_u64_max() {
    let env = Env::new(0, u64::MAX);
}

#[budget_read_bytes_lt(4_096)]
impl Contract {
    fn under_limit() {
        let env = Env::new(0, EXPECTED_COST);
    }

    fn exact_limit() {
        let env = Env::new(0, 4_096);
    }
}

fn assert_failure_contains<F, R>(case: &str, expected: &str, f: F)
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
    assert_eq!(result_error_body(), Err(TestError));
    assert_eq!(early_return_under_limit(true), Ok(()));
    assert_eq!(early_return_under_limit(false), Ok(()));
    just_below_limit();
    zero_memory_is_below_positive_limit();
    cpu_cost_does_not_use_memory_proxy();
    largest_cost_below_u64_max();
    Contract::under_limit();
}

fn run_failing_cases() {
    assert_failure_contains(
        "strictly over limit on an early return",
        "Read bytes cost (memory proxy) 8192 exceeded limit 4096",
        || early_return_body(true),
    );
    assert_failure_contains(
        "strictly over limit after falling through",
        "Read bytes cost (memory proxy) 8192 exceeded limit 4096",
        || early_return_body(false),
    );

    let message = budget_panic(Contract::exact_limit)
        .expect("the impl-block assertion should have failed at the strict boundary");
    assert!(
        message.contains("Read bytes cost (memory proxy) 4096 exceeded limit 4096"),
        "unexpected panic message: {message}"
    );
    assert!(
        message.contains("[fn `exact_limit`]"),
        "the failure must name the impl method: {message}"
    );

    assert_failure_contains(
        "maximum representable cost",
        "Read bytes cost (memory proxy) 18446744073709551615 exceeded limit 18446744073709551615",
        largest_cost_at_u64_max,
    );
}

fn main() {
    run_passing_cases();
    run_failing_cases();
}
