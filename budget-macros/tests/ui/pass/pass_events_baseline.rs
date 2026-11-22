//! `#[budget_events_lt(N, baseline = B)]` asserts the *marginal* event count and
//! names all three numbers in the failure text.
//!
//! The baseline is subtracted from `env.events().all().events().len()` before
//! the strict comparison (`saturating_sub`, then `marginal < limit`), so a
//! contract whose setup emits a known floor of events is judged on what the
//! body itself emits.
//!
//! `pass_events.rs` is not empty here and this fixture does not duplicate it:
//! it already covers the macro's body shapes, saturation, and one marginal
//! message — but only through `baseline = event_floor()`, a function-valued
//! baseline, and only for `3 measured - 1 baseline`, where the marginal figure
//! happens to be the measured count minus one. Three things were left unpinned,
//! and they are what this file adds:
//!
//! * the **literal** baseline form. `budget_events_lt(N, baseline = <literal>)`
//!   appears nowhere else in the suite (`pass_baseline.rs`, which covers the
//!   other metrics' literal baselines, has no events case), so a regression in
//!   how a literal baseline expression is spliced into the events generator
//!   would pass CI.
//! * that the subtraction **changes the verdict**. `raw_breach_passes_after_subtraction`
//!   and `without_baseline_control_panics` use the same limit and the same
//!   measurement and must disagree, so the pass is caused by the arithmetic
//!   rather than by a comparison that stopped running.
//! * the message when the marginal figure is **not** the measured count minus
//!   one: `15 measured - 5 baseline` and `20 measured - 5 baseline`, where an
//!   off-by-one or a "print the measurement" regression cannot masquerade as a
//!   true failure.
//!
//! Event counts stay small: the mock represents each event as a byte in an
//! allocated vector.

#[path = "../support/mock_env.rs"]
mod mock_env;

use budget_macros::budget_events_lt;
use mock_env::{budget_panic, Env};

// Every case below shares limit 10 and, unless named otherwise, baseline 5, so
// the verdict is decided by `measured - 5` against 10: 15 is the first count
// that fails and 14 is the last that passes.

/// 14 measured - 5 baseline = 9 marginal: below the limit, so no panic.
#[budget_events_lt(10, baseline = 5)]
fn marginal_below_limit_passes() {
    let env = Env::new_full(0, 0, 14, 0, 0);
}

/// 15 - 5 = 10 marginal, equal to the limit. The comparison is strict, so
/// equality on the *marginal* figure is already a breach.
#[budget_events_lt(10, baseline = 5)]
fn marginal_at_limit_panics() {
    let env = Env::new_full(0, 0, 15, 0, 0);
}

/// 20 - 5 = 15 marginal: the asserted number is the marginal one, not the raw
/// measurement, and both appear in the message.
#[budget_events_lt(10, baseline = 5)]
fn marginal_above_limit_panics() {
    let env = Env::new_full(0, 0, 20, 0, 0);
}

/// The case that only a working baseline can pass: 12 events measured is above
/// the limit of 10, but 12 - 5 = 7 is not.
#[budget_events_lt(10, baseline = 5)]
fn raw_breach_passes_after_subtraction() {
    let env = Env::new_full(0, 0, 12, 0, 0);
}

/// The same measurement against the same limit with no baseline, which must
/// fail — so the pass above is caused by the subtraction, not by a comparison
/// that stopped running.
#[budget_events_lt(10)]
fn without_baseline_control_panics() {
    let env = Env::new_full(0, 0, 12, 0, 0);
}

/// A measurement exactly at the baseline leaves a zero marginal count.
#[budget_events_lt(10, baseline = 5)]
fn measured_equal_to_baseline_passes() {
    let env = Env::new_full(0, 0, 5, 0, 0);
}

/// Below the baseline the marginal count saturates at zero rather than
/// wrapping: a `u64` wrap would put this 3-event body far above its limit of 1.
#[budget_events_lt(1, baseline = 5)]
fn below_baseline_saturates_to_zero() {
    let env = Env::new_full(0, 0, 3, 0, 0);
}

/// A baseline larger than the limit is still only a subtraction: 1 - 5
/// saturates to 0 and the body passes.
#[budget_events_lt(1, baseline = 5)]
fn baseline_above_limit_does_not_prevent_passing() {
    let env = Env::new_full(0, 0, 1, 0, 0);
}

fn assert_event_failure<F, R>(case: &str, expected: &str, f: F) -> String
where
    F: FnOnce() -> R + std::panic::UnwindSafe,
{
    let message = budget_panic(f)
        .unwrap_or_else(|| panic!("{case}: the event budget assertion should have failed"));
    assert!(
        message.contains(expected),
        "{case}: unexpected panic message: {message}"
    );
    message
}

fn main() {
    // Cases whose bodies must return normally.
    marginal_below_limit_passes();
    raw_breach_passes_after_subtraction();
    measured_equal_to_baseline_passes();
    below_baseline_saturates_to_zero();
    baseline_above_limit_does_not_prevent_passing();

    // The marginal figure is what the assertion compares, so it is what leads
    // the message; the parenthetical carries the raw measurement and baseline.
    assert_event_failure(
        "marginal at limit",
        "Event count 10 exceeded limit 10 (marginal: 15 measured - 5 baseline)",
        marginal_at_limit_panics,
    );
    assert_event_failure(
        "marginal above limit",
        "Event count 15 exceeded limit 10 (marginal: 20 measured - 5 baseline)",
        marginal_above_limit_panics,
    );

    // The no-baseline form of the same breach keeps the plain message, so the
    // marginal wording is emitted only when a baseline was supplied.
    let control = assert_event_failure(
        "no baseline",
        "Event count 12 exceeded limit 10",
        without_baseline_control_panics,
    );
    assert!(
        !control.contains("marginal"),
        "no-baseline failures must not claim a subtraction: {control}"
    );
}
