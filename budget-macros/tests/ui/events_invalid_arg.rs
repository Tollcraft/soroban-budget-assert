use budget_macros::budget_events_lt;

// The BudgetLimit parser expects either an integer literal or `env`/`config`
// identifiers. Passing `wrong` should produce a clear error message.
// (Optimized: Replaced unused `env` with ignored variable `_env` to avoid dead allocation warnings)
#[budget_events_lt(wrong = "500")]
fn test_invalid_arg() {
    let _env = ();
}

fn main() {}
