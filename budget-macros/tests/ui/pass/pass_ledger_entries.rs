//! `#[budget_ledger_entries_lt]` support and body-shape coverage against the
//! mock `Env`. The total asserted is reads + writes; the failure message
//! reports the breakdown.
//!
//! Ledger tracking is represented by two scalar counters, so this fixture has
//! no loops, collections, clones, or intermediate allocations to optimize. Each
//! case constructs one fixed-size `Env` and lets the macro perform its two
//! tracker reads; the body does not repeat that tracker chain just to exercise
//! the assertion, which is what previously duplicated a tracker read per case.
//!
//! `ContractCostType` stays imported even though no body names it directly: the
//! attribute expands to
//! `budget.tracker(ContractCostType::Disk{Read,Write}Entries).iterations()`, so
//! removing the import would break the expansion this file exists to check.

#[path = "../support/mock_env.rs"]
mod mock_env;

use budget_macros::budget_ledger_entries_lt;
use mock_env::{budget_panic, ContractCostType, Env};

const READ_ENTRIES: u64 = 3;
const WRITE_ENTRIES: u64 = 4;
const TOTAL_ENTRIES: u64 = READ_ENTRIES + WRITE_ENTRIES;

#[derive(Debug, PartialEq)]
struct TestError;

/// A unit-returning body checks the combined count on its normal path.
#[budget_ledger_entries_lt(50)]
fn unit_body() {
    let env = Env::new_full(0, 0, 0, READ_ENTRIES, WRITE_ENTRIES);
}

/// A trailing result is evaluated before the assertion and returned unchanged.
#[budget_ledger_entries_lt(50)]
fn result_body() -> Result<u64, TestError> {
    let env = Env::new_full(0, 0, 0, READ_ENTRIES, WRITE_ENTRIES);
    Ok(TOTAL_ENTRIES)
}

/// An error result still runs the budget assertion before it is returned.
#[budget_ledger_entries_lt(50)]
fn result_error_body() -> Result<(), TestError> {
    let env = Env::new_full(0, 0, 0, READ_ENTRIES, WRITE_ENTRIES);
    Err(TestError)
}

/// Both conditional-return paths retain the injected check.
#[budget_ledger_entries_lt(50)]
fn early_return_under_limit(exit_early: bool) -> Result<(), TestError> {
    let env = Env::new_full(0, 0, 0, READ_ENTRIES, WRITE_ENTRIES);
    if exit_early {
        return Ok(());
    }
    Ok(())
}

/// An over-limit early return must fail before returning its value.
#[budget_ledger_entries_lt(5)]
fn early_return_over_limit(exit_early: bool) -> Result<(), TestError> {
    let env = Env::new_full(0, 0, 0, READ_ENTRIES, WRITE_ENTRIES);
    if exit_early {
        return Ok(());
    }
    Ok(())
}

/// A read-only workload is measured using a zero write count.
#[budget_ledger_entries_lt(4)]
fn read_only_passes() {
    let env = Env::new_full(0, 0, 0, 3, 0);
}

/// A write-only workload is measured using a zero read count.
#[budget_ledger_entries_lt(4)]
fn write_only_passes() {
    let env = Env::new_full(0, 0, 0, 0, 3);
}

/// Zero entries are below a positive limit without requiring a special case.
#[budget_ledger_entries_lt(1)]
fn zero_entries_passes() {
    let env = Env::new_full(0, 0, 0, 0, 0);
}

/// Equality is not below the limit, so the combined strict boundary fails.
#[budget_ledger_entries_lt(4)]
fn exact_limit_panics() {
    let env = Env::new_full(0, 0, 0, 2, 2);
}

/// A combined count above the limit reports both sides and their sum.
#[budget_ledger_entries_lt(5)]
fn over_limit_panics() {
    let env = Env::new_full(0, 0, 0, READ_ENTRIES, WRITE_ENTRIES);
}

/// The saturating sum remains bounded when both tracker counts are at `u64::MAX`.
#[budget_ledger_entries_lt(18_446_744_073_709_551_615)]
fn saturating_total_panics() {
    let env = Env::new_full(0, 0, 0, u64::MAX, u64::MAX);
}

/// Assert that a ledger-budget failure contains the expected diagnostic text.
fn assert_ledger_failure<F, R>(case: &str, expected: &str, f: F)
where
    F: FnOnce() -> R + std::panic::UnwindSafe,
{
    let message = budget_panic(f)
        .unwrap_or_else(|| panic!("{case}: the ledger budget assertion should have failed"));
    assert!(
        message.contains(expected),
        "{case}: unexpected panic message: {message}"
    );
}

/// Cases whose bodies must return normally: the injected check passes.
fn run_passing_cases() {
    unit_body();
    assert_eq!(result_body(), Ok(TOTAL_ENTRIES));
    assert_eq!(result_error_body(), Err(TestError));
    assert_eq!(early_return_under_limit(true), Ok(()));
    assert_eq!(early_return_under_limit(false), Ok(()));
    read_only_passes();
    write_only_passes();
    zero_entries_passes();
}

/// Cases that must panic, each checked for the exact reported breakdown.
fn run_failing_cases() {
    assert_ledger_failure(
        "strict boundary",
        "Ledger entry count (read: 2, write: 2, total: 4) exceeded limit 4",
        exact_limit_panics,
    );
    assert_ledger_failure(
        "strictly over limit",
        "Ledger entry count (read: 3, write: 4, total: 7) exceeded limit 5",
        over_limit_panics,
    );
    assert_ledger_failure(
        "early return",
        "Ledger entry count (read: 3, write: 4, total: 7) exceeded limit 5",
        || early_return_over_limit(true),
    );
    assert_ledger_failure(
        "fall-through after conditional return",
        "Ledger entry count (read: 3, write: 4, total: 7) exceeded limit 5",
        || early_return_over_limit(false),
    );
    assert_ledger_failure(
        "saturating total",
        "Ledger entry count (read: 18446744073709551615, write: 18446744073709551615, total: 18446744073709551615) exceeded limit 18446744073709551615",
        saturating_total_panics,
    );
}

fn main() {
    run_passing_cases();
    run_failing_cases();
}
