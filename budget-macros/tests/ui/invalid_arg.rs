//! The `budget_cpu_lt` macro must reject unsupported argument names.

use budget_macros::budget_cpu_lt;

#[budget_cpu_lt(wrong = "500")]
fn test_invalid_argument() {}

fn main() {}
