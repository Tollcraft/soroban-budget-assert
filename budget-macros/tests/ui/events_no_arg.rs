use budget_macros::budget_events_lt;

// No argument should fail because the `BudgetLimit` parser expects a value.
// `#[budget_events_lt]` parses its (empty) attribute input as a
// `StandaloneSpec`, whose first step is that `BudgetLimit` parse, so the
// diagnostic is the parser's "expected an integer literal, ..." message.
// The exact diagnostic is pinned in `events_no_arg.stderr`.
//
// The rejection happens while the attribute arguments are parsed, so the
// annotated function is replaced by the emitted `compile_error!` and never
// runs. Its body is intentionally empty: it exists only to give the attribute
// something to annotate, and the previous `let env = ();` placeholder was
// dead code that could never execute.
#[budget_events_lt]
fn test_no_arg_macro_rejection() {}

fn main() {}
