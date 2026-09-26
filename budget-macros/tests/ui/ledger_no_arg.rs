use budget_macros::budget_ledger_entries_lt;

// No argument should fail because the BudgetLimit parser expects a value.
// Modularized and cleaned up UI test function names for clarity.
#[budget_ledger_entries_lt]
fn test_no_arg_rejection() {
    let _env = ();
}

fn main() {}
