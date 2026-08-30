//! `#[budget_ledger_entries_lt]` support and body-shape coverage against the
//! mock `Env`. The total asserted is reads + writes; the failure message
//! reports the breakdown.

#[path = "../support/mock_env.rs"]
mod mock_env;

use budget_macros::budget_ledger_entries_lt;
use mock_env::{budget_panic, ContractCostType, Env};

#[derive(Debug, PartialEq)]
struct TestError;

#[budget_ledger_entries_lt(50)]
fn unit_body() {
    let env = Env::new_full(0, 0, 0, 3, 4);
    let _ = env
        .cost_estimate()
        .budget()
        .tracker(ContractCostType::DiskReadEntries)
        .iterations();
}

#[budget_ledger_entries_lt(50)]
fn result_body() -> Result<u64, TestError> {
    let env = Env::new_full(0, 0, 0, 3, 4);
    Ok(env
        .cost_estimate()
        .budget()
        .tracker(ContractCostType::DiskReadEntries)
        .iterations())
}

#[budget_ledger_entries_lt(5)]
fn over_limit_panics() {
    let env = Env::new_full(0, 0, 0, 3, 4);
    let _ = env
        .cost_estimate()
        .budget()
        .tracker(ContractCostType::DiskReadEntries)
        .iterations();
}

fn main() {
    unit_body();
    assert_eq!(result_body(), Ok(3));

    let message =
        budget_panic(over_limit_panics).expect("the ledger assertion should have failed");
    assert!(
        message.contains("Ledger entry count (read: 3, write: 4, total: 7) exceeded limit 5"),
        "unexpected panic message: {message}"
    );
}
