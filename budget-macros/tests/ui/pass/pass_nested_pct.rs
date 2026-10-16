//! Nested percentage limit inside `#[budget_lt(...)]`: `cpu = pct = N, of = …`
//! combined with a sibling literal `mem` limit.
//!
//! `PCT_TEST_CPU_NETWORK` is `10_000` in the fixture's env file, so the
//! quarter CPU limit resolves to `2_500` (`reference * pct / 100`) while the
//! `mem` limit stays a literal `500`. The cases prove the hand-off from
//! `BudgetLimit::parse` back to `BudgetSpec::parse` works: the CPU assertion
//! reports the resolved percentage limit and the sibling `mem` assertion
//! still fires.

#[path = "../support/mock_env.rs"]
mod mock_env;

use budget_macros::budget_lt;
use mock_env::{budget_panic, Env};

const PCT_ENV_FILE: &str = "tests/ui/support/pass_env_file.env";

/// Both metrics under their limits must pass.
#[budget_lt(
    cpu = pct = 25,
    of = env_file = PCT_ENV_FILE,
    env = "PCT_TEST_CPU_NETWORK",
    mem = 500
)]
fn nested_pct_both_pass() {
    let env = Env::new(2_499, 400);
}

/// A CPU cost above the resolved quarter limit must fail with the cost, the
/// resolved limit, and the percentage in its diagnostic.
#[budget_lt(
    cpu = pct = 25,
    of = env_file = PCT_ENV_FILE,
    env = "PCT_TEST_CPU_NETWORK",
    mem = 500
)]
fn nested_pct_cpu_fails() {
    let env = Env::new(2_501, 400);
}

/// A memory cost above the literal sibling limit must fail even when the CPU
/// cost is under its percentage limit, proving both metrics are checked.
#[budget_lt(
    cpu = pct = 25,
    of = env_file = PCT_ENV_FILE,
    env = "PCT_TEST_CPU_NETWORK",
    mem = 500
)]
fn nested_pct_mem_fails() {
    let env = Env::new(100, 600);
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

/// Costs that resolve below their limits and must pass.
fn run_passing_cases() {
    nested_pct_both_pass();
}

/// Costs that land above either limit and must panic.
fn run_failing_cases() {
    assert_pct_failure("nested cpu over quarter limit", 2_501, 2_500, 25, nested_pct_cpu_fails);
    let mem_message =
        budget_panic(nested_pct_mem_fails).expect("the mem assertion should have failed");
    assert!(
        mem_message.contains("Memory bytes cost 600 exceeded limit 500"),
        "sibling mem limit must still be enforced: {mem_message}"
    );
}

fn main() {
    run_passing_cases();
    run_failing_cases();
}
