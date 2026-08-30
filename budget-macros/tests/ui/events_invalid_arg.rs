use budget_macros::budget_events_lt;

// The BudgetLimit parser expects either an integer literal or `env`/`config`
// identifiers. Passing `wrong` should produce a clear error message.
#[budget_events_lt(wrong = "500")]
fn test_invalid_arg() {
    let env = ();
}

fn main() {}
