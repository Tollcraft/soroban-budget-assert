//! Percentage-based budget limits: `pct = N, of = env_file = "...", env = "KEY"`.
//!
//! The reference limit is read from the named key in an env file at test
//! runtime. The resolved limit is calculated with integer arithmetic
//! (`reference * pct / 100`), so a fractional limit is truncated before the
//! assertion runs. The comparison is strict: a cost equal to the resolved
//! limit fails, not just a cost above it.
//!
//! `PCT_TEST_CPU_NETWORK` is `10_000` in the fixture's env file. Thus the
//! quarter limit is `2_500`, and the cases below cover just-below, exact, and
//! over-limit values as well as the valid 50% and 100% boundaries.

#[path = "../support/mock_env.rs"]
mod mock_env;

use budget_macros::budget_cpu_lt;
use mock_env::{budget_panic, Env};

const PCT_ENV_FILE: &str = "tests/ui/support/pass_env_file.env";
const NETWORK_CPU_LIMIT: u64 = 10_000;

/// A cost one unit below the quarter limit passes.
#[budget_cpu_lt(pct = 25, of = env_file = PCT_ENV_FILE, env = "PCT_TEST_CPU_NETWORK")]
fn pct_passes_when_under() {
    let env = Env::new(2_499, 0);
    let _ = env.cost_estimate().budget().cpu_instruction_cost();
}

/// A cost above the quarter limit fails with all resolved values in its diagnostic.
#[budget_cpu_lt(pct = 25, of = env_file = PCT_ENV_FILE, env = "PCT_TEST_CPU_NETWORK")]
fn pct_fails_when_over() {
    let env = Env::new(2_501, 0);
    let _ = env.cost_estimate().budget().cpu_instruction_cost();
}

/// Equality is not below the limit, so the exact quarter boundary must fail.
#[budget_cpu_lt(pct = 25, of = env_file = PCT_ENV_FILE, env = "PCT_TEST_CPU_NETWORK")]
fn pct_fails_at_limit() {
    let env = Env::new(NETWORK_CPU_LIMIT / 4, 0);
    let _ = env.cost_estimate().budget().cpu_instruction_cost();
}

/// A cost below the half limit passes.
#[budget_cpu_lt(pct = 50, of = env_file = PCT_ENV_FILE, env = "PCT_TEST_CPU_NETWORK")]
fn pct_50_passes() {
    let env = Env::new(4_999, 0);
    let _ = env.cost_estimate().budget().cpu_instruction_cost();
}

/// A cost below the full network limit passes.
#[budget_cpu_lt(pct = 100, of = env_file = PCT_ENV_FILE, env = "PCT_TEST_CPU_NETWORK")]
fn pct_100_passes() {
    let env = Env::new(9_999, 0);
    let _ = env.cost_estimate().budget().cpu_instruction_cost();
}

/// Verify that percentage failures identify the measured cost, resolved limit,
/// and percentage that produced the limit.
fn assert_pct_failure<F, R>(case: &str, expected_cost: u64, expected_limit: u64, pct: u64, f: F)
where
    F: FnOnce() -> R + std::panic::UnwindSafe,
{
    let message = budget_panic(f)
        .unwrap_or_else(|| panic!("{case}: the percentage budget assertion should have failed"));
    let expected_cost = expected_cost.to_string();
    let expected_limit = expected_limit.to_string();
    assert!(
        message.contains(expected_cost.as_str()),
        "{case}: failure message must contain the actual cost: {message}"
    );
    assert!(
        message.contains(expected_limit.as_str()),
        "{case}: failure message must contain the resolved limit: {message}"
    );
    assert!(
        message.contains(&format!("{pct}%")),
        "{case}: failure message must contain the percentage: {message}"
    );
}

/// Costs that resolve below their percentage limit and must pass.
fn run_passing_cases() {
    pct_passes_when_under();
    pct_50_passes();
    pct_100_passes();
}

/// Costs that land at or above their percentage limit and must panic.
fn run_failing_cases() {
    assert_pct_failure("over quarter limit", 2_501, 2_500, 25, pct_fails_when_over);
    assert_pct_failure("at quarter limit", 2_500, 2_500, 25, pct_fails_at_limit);
}

fn main() {
    run_passing_cases();
    run_failing_cases();
}
