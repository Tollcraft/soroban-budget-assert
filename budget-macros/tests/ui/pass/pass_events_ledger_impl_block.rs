//! `#[budget_events_lt]` and `#[budget_ledger_entries_lt]` on an `impl` block.
//!
//! Issue #988: both macros read a single `fn` only, so a contract module that
//! annotates every other budget macro at block level had to keep writing the
//! event and ledger-entry checks method by method. They now expand through the
//! same `impl`-block path as the rest of the set: the block limit applies to
//! every method that does not carry its own budget attribute, and the failure
//! message names the offending method.
//!
//! Covered here:
//!   * block-level limits, strict boundaries, and failures on both the normal
//!     and the early-return exit path;
//!   * block baselines, with the `[fn …]` label in the marginal message;
//!   * per-method overrides, including an override that switches metric;
//!   * that a method-level attribute alone still produces its unlabelled
//!     message, because the label is what a block expansion adds.
//!
//! NOTE (pre-existing, not introduced here): two *different* budget attributes
//! stacked at block level on one `impl` (`#[budget_events_lt(4)]` above
//! `#[budget_ledger_entries_lt(6)]`) fail with
//! `error[E0658]: attributes on expressions are experimental`. The block
//! expansion re-emits the sibling attribute in front of the block it is
//! expanding, and rustc then re-parses that fragment as an expression. It
//! affects all eight attributes equally, so annotate one metric per block and
//! use a method-level attribute for the other.

#[path = "../support/mock_env.rs"]
mod mock_env;

use budget_macros::{budget_cpu_lt, budget_events_lt, budget_ledger_entries_lt};
// `soroban_sdk` looks unused but is not: the ledger-entry expansion emits the
// fully-qualified `soroban_sdk::testutils::budget::ContractCostType` path,
// resolved through the mock's shim re-export.
use mock_env::{budget_panic, Env, soroban_sdk};

#[derive(Debug, PartialEq)]
struct TestError;

struct EventContract;
struct EventBaselineContract;
struct LedgerContract;
struct LedgerBaselineContract;

fn floor() -> u64 {
    2
}

#[budget_events_lt(4)]
impl EventContract {
    const BLOCK_LIMIT: u64 = 4;

    fn within_block_limit() {
        let env = Env::new_full(0, 0, 3, 0, 0);
        let _ = env.events().all();
    }

    fn over_block_limit() {
        let env = Env::new_full(0, 0, 5, 0, 0);
        let _ = env.events().all();
    }

    // Over the block's 4 but under this method's own 20, so it passes: the
    // method carries its own attribute and the block skips it.
    #[budget_events_lt(20)]
    fn override_lifts_the_block_limit() {
        let env = Env::new_full(0, 0, 19, 0, 0);
        let _ = env.events().all();
    }

    #[budget_cpu_lt(1_000)]
    fn override_switches_metric() {
        // 500 events would breach the block's event limit; the method's own CPU
        // limit is what runs here.
        let env = Env::new_full(900, 0, 500, 0, 0);
        let _ = env.cost_estimate().budget().cpu_instruction_cost();
    }

    // Strict boundary for the method's own limit, not the block's.
    #[budget_events_lt(8)]
    fn at_its_own_override_limit() {
        let env = Env::new_full(0, 0, 8, 0, 0);
        let _ = env.events().all();
    }

    fn trailing_result() -> Result<u64, TestError> {
        let env = Env::new_full(0, 0, 2, 0, 0);
        Ok(env.events().all().events().len() as u64)
    }

    fn under_limit_on_early_return(exit_early: bool) -> Result<(), TestError> {
        let env = Env::new_full(0, 0, 3, 0, 0);
        if exit_early {
            return Ok(());
        }
        Ok(())
    }

    fn over_limit_on_both_returns(exit_early: bool) -> Result<(), TestError> {
        let env = Env::new_full(0, 0, 9, 0, 0);
        if exit_early {
            return Ok(());
        }
        Ok(())
    }
}

#[budget_events_lt(2, baseline = floor())]
impl EventBaselineContract {
    fn within_limit_after_baseline() {
        let env = Env::new_full(0, 0, 3, 0, 0);
        let _ = env.events().all();
    }

    fn over_limit_after_baseline() {
        let env = Env::new_full(0, 0, 5, 0, 0);
        let _ = env.events().all();
    }
}

#[budget_ledger_entries_lt(6)]
impl LedgerContract {
    fn within_block_limit() {
        let env = Env::new_full(0, 0, 0, 4, 1);
        let _ = env.cost_estimate().budget().cpu_instruction_cost();
    }

    fn over_block_limit() {
        let env = Env::new_full(0, 0, 0, 5, 2);
        let _ = env.cost_estimate().budget().cpu_instruction_cost();
    }

    fn at_the_block_boundary() {
        let env = Env::new_full(0, 0, 0, 3, 3);
        let _ = env.cost_estimate().budget().cpu_instruction_cost();
    }

    #[budget_ledger_entries_lt(20)]
    fn override_lifts_the_block_limit() {
        let env = Env::new_full(0, 0, 0, 12, 7);
        let _ = env.cost_estimate().budget().cpu_instruction_cost();
    }

    fn over_limit_on_early_return(exit_early: bool) -> Result<(), TestError> {
        let env = Env::new_full(0, 0, 0, 7, 4);
        if exit_early {
            return Ok(());
        }
        Ok(())
    }
}

#[budget_ledger_entries_lt(4, baseline = floor())]
impl LedgerBaselineContract {
    fn within_limit_after_baseline() {
        let env = Env::new_full(0, 0, 0, 3, 2);
        let _ = env.cost_estimate().budget().cpu_instruction_cost();
    }

    fn read_only_over_limit_after_baseline() {
        let env = Env::new_full(0, 0, 0, 6, 0);
        let _ = env.cost_estimate().budget().cpu_instruction_cost();
    }

    fn write_only_over_limit_after_baseline() {
        let env = Env::new_full(0, 0, 0, 0, 7);
        let _ = env.cost_estimate().budget().cpu_instruction_cost();
    }
}

/// Assert that a block-expanded failure reports the expected measurement text
/// *and* names the method that caused it.
fn assert_named_failure<F, R>(case: &str, expected: &str, method: &str, f: F)
where
    F: FnOnce() -> R + std::panic::UnwindSafe,
{
    let message = budget_panic(f)
        .unwrap_or_else(|| panic!("{case}: the budget assertion should have failed"));
    assert!(
        message.contains(expected),
        "{case}: unexpected panic message: {message}"
    );
    assert!(
        message.contains(&format!("[fn `{method}`]")),
        "{case}: the failure must name `{method}`: {message}"
    );
}

fn main() {
    assert_eq!(EventContract::BLOCK_LIMIT, 4);

    // ---- events, block level ----
    EventContract::within_block_limit();
    EventContract::override_lifts_the_block_limit();
    EventContract::under_limit_on_early_return(true);
    EventContract::under_limit_on_early_return(false);
    assert_eq!(EventContract::trailing_result(), Ok(2));

    assert_named_failure(
        "event block breach",
        "Event count 5 exceeded limit 4",
        "over_block_limit",
        EventContract::over_block_limit,
    );
    assert_named_failure(
        "event breach on the early-return path",
        "Event count 9 exceeded limit 4",
        "over_limit_on_both_returns",
        || EventContract::over_limit_on_both_returns(true),
    );
    assert_named_failure(
        "event breach on the fall-through path",
        "Event count 9 exceeded limit 4",
        "over_limit_on_both_returns",
        || EventContract::over_limit_on_both_returns(false),
    );

    // A method-level attribute is expanded on the method alone, so it has no
    // block to disambiguate within and its message carries no `[fn …]` label —
    // the label is what a block-level expansion adds.
    let override_message = budget_panic(EventContract::at_its_own_override_limit)
        .expect("a method's own event limit should be strict");
    assert!(
        override_message.contains("Event count 8 exceeded limit 8"),
        "the method's own limit should be enforced: {override_message}"
    );
    assert!(
        !override_message.contains("[fn"),
        "a method-level attribute has no block context to label: {override_message}"
    );

    // The method-level CPU override ran instead of the block's event limit.
    EventContract::override_switches_metric();

    // ---- events, block baseline ----
    EventBaselineContract::within_limit_after_baseline();
    assert_named_failure(
        "event baseline breach",
        "Event count 3 exceeded limit 2 [fn `over_limit_after_baseline`] \
         (marginal: 5 measured - 2 baseline)",
        "over_limit_after_baseline",
        EventBaselineContract::over_limit_after_baseline,
    );

    // ---- ledger entries, block level ----
    LedgerContract::within_block_limit();
    LedgerContract::override_lifts_the_block_limit();

    assert_named_failure(
        "ledger block breach",
        "Ledger entry count (read: 5, write: 2, total: 7) exceeded limit 6",
        "over_block_limit",
        LedgerContract::over_block_limit,
    );
    assert_named_failure(
        "ledger strict boundary",
        "Ledger entry count (read: 3, write: 3, total: 6) exceeded limit 6",
        "at_the_block_boundary",
        LedgerContract::at_the_block_boundary,
    );
    assert_named_failure(
        "ledger breach on the early-return path",
        "Ledger entry count (read: 7, write: 4, total: 11) exceeded limit 6",
        "over_limit_on_early_return",
        || LedgerContract::over_limit_on_early_return(true),
    );

    // ---- ledger entries, block baseline ----
    LedgerBaselineContract::within_limit_after_baseline();
    assert_named_failure(
        "ledger read-only baseline breach",
        "Ledger entry count (read: 6, write: 0, total: 6) exceeded limit 4 \
         [fn `read_only_over_limit_after_baseline`] (marginal: 4 measured - 2 baseline)",
        "read_only_over_limit_after_baseline",
        LedgerBaselineContract::read_only_over_limit_after_baseline,
    );
    assert_named_failure(
        "ledger write-only baseline breach",
        "Ledger entry count (read: 0, write: 7, total: 7) exceeded limit 4 \
         [fn `write_only_over_limit_after_baseline`] (marginal: 5 measured - 2 baseline)",
        "write_only_over_limit_after_baseline",
        LedgerBaselineContract::write_only_over_limit_after_baseline,
    );
}
