use budget_macros::budget_read_bytes_lt;

// `budget_read_bytes_lt` with no argument must be rejected the same way its
// siblings are: the limit parser has nothing to read. Like the other
// single-metric macros it parses a `StandaloneSpec`, whose first step is a
// `BudgetLimit` parse, so an empty attribute collapses to the parser's
// "expected an integer literal, ..." message, pinned in `read_bytes_no_arg.stderr`.
//
// The rejection happens while the attribute arguments are parsed, so the
// annotated function is replaced by the emitted `compile_error!` and never
// runs. Its body is intentionally empty: it exists only to give the attribute
// something to annotate, and the previous `let env = ();` placeholder was
// dead code that could never execute.
#[budget_read_bytes_lt]
fn test_read_bytes_no_arg() {}

fn main() {}