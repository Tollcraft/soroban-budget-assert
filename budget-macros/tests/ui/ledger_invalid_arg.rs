use budget_macros::budget_ledger_entries_lt;

// An unknown key inside the attribute must be rejected loudly, with the
// offending identifier named rather than a generic "expected one of …".
// `budget_ledger_entries_lt` parses a `StandaloneSpec`, whose `BudgetLimit`
// step accepts an integer literal or an `env` / `env_file` / `config` / `pct`
// source key — `wrong` is none of those, so the parser reports exactly that:
// "expected `env`, `env_file`, `config`, or `pct`, got `wrong`".
// The exact diagnostic is pinned in `ledger_invalid_arg.stderr`.
//
// The rejection happens while the attribute arguments are parsed, so the
// annotated function is replaced by the emitted `compile_error!` and never
// runs. Its body is intentionally empty: the previous `let env = ();`
// placeholder was dead code that could never execute.
#[budget_ledger_entries_lt(wrong = "500")]
fn test_invalid_arg() {}

fn main() {}