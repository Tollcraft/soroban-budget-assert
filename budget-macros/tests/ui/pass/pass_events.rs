//! `#[budget_events_lt]` support and body-shape coverage against the mock `Env`.
//!
//! The event count is read from `env.events().all().events().len()` — a real
//! count, exercised here against the mock. The cases cover unit, result,
//! early-return, and error-return bodies; zero and boundary counts; strict
//! failure behavior; and baseline subtraction. Event counts stay small because
//! the mock represents each event with a byte in an allocated vector.

#[path = "../support/mock_env.rs"]
mod mock_env;

use budget_macros::budget_events_lt;
use mock_env::{budget_panic, Env};

const EXPECTED_EVENT_COUNT: u64 = 3;

#[derive(Debug, PartialEq)]
struct TestError;

/// A function whose ordinary fall-through path is checked after the body runs.
#[budget_events_lt(10)]
fn unit_body() {
    let env = Env::new_full(0, 0, 3, 0, 0);
}

/// A trailing result is evaluated before the injected assertion and returned
/// unchanged after the check passes.
#[budget_events_lt(10)]
fn result_body() -> Result<u64, TestError> {
    let env = Env::new_full(0, 0, 3, 0, 0);
    Ok(EXPECTED_EVENT_COUNT)
}

/// An error result still goes through the assertion before it is returned.
#[budget_events_lt(10)]
fn result_error_body() -> Result<(), TestError> {
    let env = Env::new_full(0, 0, 3, 0, 0);
    Err(TestError)
}

/// Both branches of an early-return body must retain the budget check.
#[budget_events_lt(10)]
fn early_return_under_limit(exit_early: bool) -> Result<(), TestError> {
    let env = Env::new_full(0, 0, 3, 0, 0);
    if exit_early {
        return Ok(());
    }
    Ok(())
}

/// An early return over the limit must fail before returning its value.
#[budget_events_lt(2)]
fn early_return_over_limit(exit_early: bool) -> Result<(), TestError> {
    let env = Env::new_full(0, 0, 5, 0, 0);
    if exit_early {
        return Ok(());
    }
    Ok(())
}

/// Zero is below every positive limit and must pass without a special case.
#[budget_events_lt(1)]
fn zero_events_passes() {
    let env = Env::new_full(0, 0, 0, 0, 0);
}

/// A cost one event below the limit is the largest passing integer boundary.
#[budget_events_lt(4)]
fn just_below_limit_passes() {
    let env = Env::new_full(0, 0, 3, 0, 0);
}

/// The limit is strict, so equality is already a failure.
#[budget_events_lt(4)]
fn exact_limit_panics() {
    let env = Env::new_full(0, 0, 4, 0, 0);
}

/// A cost above the limit must report the event count in its diagnostic.
#[budget_events_lt(2)]
fn over_limit_panics() {
    let env = Env::new_full(0, 0, 5, 0, 0);
}

/// A baseline removes a known setup cost before comparing the event count.
#[budget_events_lt(2, baseline = event_floor())]
fn baseline_passes() {
    let env = Env::new_full(0, 0, 2, 0, 0);
}

/// A baseline does not disable the strict marginal-cost comparison.
#[budget_events_lt(2, baseline = event_floor())]
fn baseline_at_limit_panics() {
    let env = Env::new_full(0, 0, 3, 0, 0);
}

/// Subtraction saturates at zero when the measured count is below the floor.
#[budget_events_lt(1, baseline = event_floor())]
fn baseline_saturates_below_floor() {
    let env = Env::new_full(0, 0, 0, 0, 0);
}

/// Stand-in for a known event-emission floor.
fn event_floor() -> u64 {
    1
}

/// Assert that an event-budget failure contains the expected diagnostic text.
fn assert_event_failure<F, R>(case: &str, expected: &str, f: F)
where
    F: FnOnce() -> R + std::panic::UnwindSafe,
{
    let message = budget_panic(f)
        .unwrap_or_else(|| panic!("{case}: the event budget assertion should have failed"));
    assert!(
        message.contains(expected),
        "{case}: unexpected panic message: {message}"
    );
}

/// Cases whose bodies must return normally: the injected check passes.
fn run_passing_cases() {
    unit_body();
    assert_eq!(result_body(), Ok(EXPECTED_EVENT_COUNT));
    assert_eq!(result_error_body(), Err(TestError));
    assert_eq!(early_return_under_limit(true), Ok(()));
    assert_eq!(early_return_under_limit(false), Ok(()));
    zero_events_passes();
    just_below_limit_passes();
    baseline_passes();
    baseline_saturates_below_floor();
}

/// Cases that must panic, each checked for the exact reported event count.
fn run_failing_cases() {
    assert_event_failure(
        "strict boundary",
        "Event count 4 exceeded limit 4",
        exact_limit_panics,
    );
    assert_event_failure(
        "strictly over limit",
        "Event count 5 exceeded limit 2",
        over_limit_panics,
    );
    assert_event_failure(
        "early return",
        "Event count 5 exceeded limit 2",
        || early_return_over_limit(true),
    );
    assert_event_failure(
        "fall-through after conditional return",
        "Event count 5 exceeded limit 2",
        || early_return_over_limit(false),
    );
    assert_event_failure(
        "baseline strict boundary",
        "Event count 2 exceeded limit 2 (marginal: 3 measured - 1 baseline)",
        baseline_at_limit_panics,
    );
}

fn main() {
    run_passing_cases();
    run_failing_cases();
}
