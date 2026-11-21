//! `#[budget_ledger_entries_lt]` supports `baseline = <expr>` exactly as its
//! siblings do: the read and write entry counts are summed first, then the
//! baseline is subtracted (saturating), and the *marginal* total is what gets
//! compared. `pass_ledger_entries.rs` covers the literal-limit paths only, so
//! this is the ledger case of baseline subtraction, kept in its own file so it
//! does not touch fixtures other issues also edit.
//!
//! No `ContractCostType` import is needed: the attribute expands to the
//! fully-qualified
//! `soroban_sdk::testutils::budget::ContractCostType::Disk{Read,Write}Entries`
//! path, resolved here through the `soroban_sdk` shim re-exported by the mock.

#[path = "../support/mock_env.rs"]
mod mock_env;

use budget_macros::budget_ledger_entries_lt;
use mock_env::{budget_panic, Env, soroban_sdk};

/// Entries the fixture treats as unavoidable setup cost.
fn setup_floor() -> u64 {
    2
}

/// 5 measured - 2 baseline = 3 marginal, under the 4 limit.
#[budget_ledger_entries_lt(4, baseline = setup_floor())]
fn within_limit_after_baseline() {
    let env = Env::new_full(0, 0, 0, 3, 2);
}

/// A measurement below the baseline saturates to 0 rather than wrapping.
#[budget_ledger_entries_lt(1, baseline = setup_floor())]
fn below_baseline_saturates() {
    let env = Env::new_full(0, 0, 0, 1, 1);
}

/// 5 measured - 2 baseline = 3 marginal against a limit of 3: equality is not
/// below the limit, and the report keeps the raw breakdown next to the marginal
/// figures so the subtraction is auditable.
#[budget_ledger_entries_lt(3, baseline = setup_floor())]
fn exact_marginal_limit_panics() {
    let env = Env::new_full(0, 0, 0, 3, 2);
}

/// 7 measured - 2 baseline = 5 marginal, over the 3 limit: the baseline shifts
/// the comparison, it does not disable it.
#[budget_ledger_entries_lt(3, baseline = setup_floor())]
fn over_limit_after_baseline() {
    let env = Env::new_full(0, 0, 0, 4, 3);
}

fn main() {
    within_limit_after_baseline();
    below_baseline_saturates();

    let boundary = budget_panic(exact_marginal_limit_panics)
        .expect("the marginal ledger assertion should have failed");
    assert!(
        boundary.contains(
            "Ledger entry count (read: 3, write: 2, total: 5) exceeded limit 3 \
             (marginal: 3 measured - 2 baseline)"
        ),
        "the strict boundary should report the marginal figure it compared: {boundary}"
    );

    let over = budget_panic(over_limit_after_baseline)
        .expect("the marginal ledger assertion should have failed");
    assert!(
        over.contains(
            "Ledger entry count (read: 4, write: 3, total: 7) exceeded limit 3 \
             (marginal: 5 measured - 2 baseline)"
        ),
        "the baseline should not disable the comparison: {over}"
    );
}
