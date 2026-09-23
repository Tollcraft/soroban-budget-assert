use budget_macros::budget_cpu_lt;

// No argument should fail because the `BudgetLimit` parser expects a value.
// `#[budget_cpu_lt]` parses its (empty) attribute input as a `StandaloneSpec`,
// whose first step is that `BudgetLimit` parse, so the diagnostic is the
// parser's "expected an integer literal, ..." message. The exact diagnostic is
// pinned in `no_arg.stderr`.
//
// The parser behaviour behind it is additionally covered by the
// `budget_macros::tests` unit tests in `src/lib.rs`.
#[budget_cpu_lt]
fn test_no_arg() {}

fn main() {}
