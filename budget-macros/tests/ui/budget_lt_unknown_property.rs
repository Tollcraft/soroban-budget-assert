//! The `budget_lt` macro must reject unsupported configuration keys.

use budget_macros::budget_lt;

#[budget_lt(unknown_prop = 1000)]
fn test_unknown_property() {}

fn main() {}
