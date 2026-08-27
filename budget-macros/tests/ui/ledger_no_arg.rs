use budget_macros::budget_ledger_entries_lt;

// No argument should fail because the BudgetLimit parser expects a value.
#[budget_ledger_entries_lt]
fn test_no_arg() {
    let env = ();
}

fn main() {}
