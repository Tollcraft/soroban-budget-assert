//! The exact failure text `#[budget_ledger_entries_lt]` produces, including
//! the read/write breakdown (issue #1000).
//!
//! `generate_ledger_assert` reads `DiskReadEntries` and `DiskWriteEntries`
//! separately, adds them with `saturating_add`, and its panic reports all
//! three numbers plus the limit. `pass_ledger_entries.rs` exercises the
//! attribute over every body shape and checks each message with `contains`
//! on a fragment; nothing pins the message as a whole, so a reworded or
//! re-ordered breakdown — or a total computed once and reused where a side
//! belongs — stays green while every reader of the diagnostic (and every
//! recorded budget report) silently changes.
//!
//! The read and write counts are deliberately asymmetric in each breaching
//! case, and each expected string is compared in full, so swapping the two
//! sides, dropping one, or reporting the raw measurement where the marginal
//! one belongs fails here rather than in review.

#[path = "../support/mock_env.rs"]
mod mock_env;

use budget_macros::budget_ledger_entries_lt;
use mock_env::{budget_panic, Env, soroban_sdk};

/// Stand-in floor subtracted from the combined count in the baseline cases.
fn floor() -> u64 {
    5
}

/// Ten entries against a limit of nine, with the two sides asymmetric (7 reads,
/// 3 writes) so the breakdown identifies both.
#[budget_ledger_entries_lt(9)]
fn ten_entries_over_limit() {
    let env = Env::new_full(0, 0, 0, 7, 3);
}

/// The same ten entries against a limit of one, with a floor of five: the
/// headline count is the total, and the marginal arithmetic is spelled out
/// beside it.
#[budget_ledger_entries_lt(1, baseline = floor())]
fn ten_entries_over_marginal_limit() {
    let env = Env::new_full(0, 0, 0, 7, 3);
}

/// Three entries measured against a floor of five saturate to zero marginal,
/// so they stay under the limit instead of wrapping around `u64`.
#[budget_ledger_entries_lt(4, baseline = floor())]
fn below_the_baseline_saturates() {
    let env = Env::new_full(0, 0, 0, 2, 1);
}

/// The combined count saturates at `u64::MAX` rather than wrapping to one, and
/// the two tracker sides are still reported on their own.
#[budget_ledger_entries_lt(18_446_744_073_709_551_615)]
fn saturating_total_at_the_limit() {
    let env = Env::new_full(0, 0, 0, u64::MAX, 1);
}

/// Compare a ledger failure's message with the full expected text, so no part
/// of the breakdown can drift.
fn assert_exact_message<F, R>(case: &str, expected: &str, f: F)
where
    F: FnOnce() -> R + std::panic::UnwindSafe,
{
    let message = budget_panic(f).unwrap_or_else(|| panic!("{case}: should have failed"));
    assert_eq!(
        message, expected,
        "{case}: the breakdown message must match in full"
    );
}

fn main() {
    assert_exact_message(
        "ten entries over limit 9",
        "Ledger entry count (read: 7, write: 3, total: 10) exceeded limit 9 \
         - local estimate, real network entry counts may differ",
        ten_entries_over_limit,
    );
    assert_exact_message(
        "ten entries over marginal limit 1",
        "Ledger entry count (read: 7, write: 3, total: 10) exceeded limit 1 \
         (marginal: 5 measured - 5 baseline) \
         - local estimate, real network entry counts may differ",
        ten_entries_over_marginal_limit,
    );
    below_the_baseline_saturates();
    assert_exact_message(
        "saturating total at the u64 limit",
        "Ledger entry count (read: 18446744073709551615, write: 1, \
         total: 18446744073709551615) exceeded limit 18446744073709551615 \
         - local estimate, real network entry counts may differ",
        saturating_total_at_the_limit,
    );
}
