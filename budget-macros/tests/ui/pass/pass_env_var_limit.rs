//! The `env = "VAR"` limit form keeps working, including for bodies that set the
//! variable themselves (the limit is still read after the body has run) and for
//! bodies that end in a trailing expression.
//!
//! As in `pass_mem.rs`, the pass/failure orchestration is split into focused
//! helpers (`run_passing_cases` / `run_failing_cases` / `assert_limit_exceeded`)
//! so each scenario stays independently reviewable as the case matrix grows.

#[path = "../support/mock_env.rs"]
mod mock_env;

use budget_macros::{budget_cpu_lt, budget_mem_lt};
use mock_env::{budget_panic, Env};

/// The diagnostic every exceeded-CPU case must produce: both the env-provided
/// limit (1000) and the mock cost (1001) are named.
const CPU_LIMIT_EXCEEDED: &str = "CPU instruction cost 1001 exceeded limit 1000";

#[derive(Debug, PartialEq)]
struct TestError;

#[budget_cpu_lt(env = "UI_TEST_MAX_CPU")]
fn limit_set_inside_body() -> Result<(), TestError> {
    std::env::set_var("UI_TEST_MAX_CPU", "1000");
    let env = Env::new(999, 0);
    Ok(())
}

#[budget_cpu_lt(env = "UI_TEST_MAX_CPU_EXCEEDED")]
fn limit_exceeded() {
    std::env::set_var("UI_TEST_MAX_CPU_EXCEEDED", "1000");
    let env = Env::new(1_001, 0);
}

/// The macro injects a `budget_env_resolve` helper that the test body may shadow
/// (`amm-pool-contract`'s `env = "VAR"` tests do exactly this). The limit is
/// resolved in body scope at the exit point, so the shadowing binding wins: an
/// unset `UI_TEST_SHADOWED` would otherwise mean "no limit" and never panic.
#[budget_cpu_lt(env = "UI_TEST_SHADOWED")]
fn limit_from_shadowed_resolver() -> Result<(), TestError> {
    let budget_env_resolve = |_var: &str| -> Option<String> { Some("1000".to_string()) };
    let env = Env::new(1_001, 0);
    Ok(())
}

/// Same, reached through an early `return` instead of the trailing expression.
#[budget_cpu_lt(env = "UI_TEST_SHADOWED")]
fn shadowed_resolver_on_early_return(exit_early: bool) {
    let budget_env_resolve = |_var: &str| -> Option<String> { Some("1000".to_string()) };
    let env = Env::new(1_001, 0);
    if exit_early {
        return;
    }
}

/// An unset (or unparsable) variable falls back to `u64::MAX`, i.e. no limit.
#[budget_mem_lt(env = "UI_TEST_MAX_MEM_UNSET")]
fn limit_missing_falls_back() -> Result<u64, TestError> {
    std::env::remove_var("UI_TEST_MAX_MEM_UNSET");
    let env = Env::new(0, u64::MAX - 1);
    Ok(env.cost_estimate().budget().memory_bytes_cost())
}

/// Runs `case`, requiring it to panic with the exceeded-CPU diagnostic.
fn assert_limit_exceeded<F, R>(case: &str, f: F)
where
    F: FnOnce() -> R + std::panic::UnwindSafe,
{
    let message = budget_panic(f)
        .unwrap_or_else(|| panic!("{case}: the budget assertion should have failed"));
    assert!(
        message.contains(CPU_LIMIT_EXCEEDED),
        "{case}: unexpected panic message: {message}"
    );
}

/// Bodies that stay within the env-configured limit must not panic and must
/// return their value unchanged.
fn run_passing_cases() {
    assert_eq!(limit_set_inside_body(), Ok(()));
    assert_eq!(limit_missing_falls_back(), Ok(u64::MAX - 1));
}

/// Every path that resolves the limit to 1000 while the mock costs 1001 must
/// panic: whether the limit comes from the body, a shadowed resolver, or an
/// early return.
fn run_failing_cases() {
    assert_limit_exceeded("limit_exceeded", limit_exceeded);
    assert_limit_exceeded("limit_from_shadowed_resolver", || {
        limit_from_shadowed_resolver().map(|_| ())
    });
    assert_limit_exceeded("shadowed_resolver_on_early_return", || {
        shadowed_resolver_on_early_return(true)
    });
}

fn main() {
    run_passing_cases();
    run_failing_cases();
}
