use budget_macros::budget_events_lt;

// No argument should fail because the BudgetLimit parser expects a value.
#[budget_events_lt]
fn test_no_arg() {
    let env = ();
}

fn main() {}
