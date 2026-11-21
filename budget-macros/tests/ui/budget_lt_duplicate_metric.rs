//! `budget_lt` must reject a metric named twice instead of keeping the last.

use budget_macros::budget_lt;

// error-pattern: `cpu` cannot be specified more than once

#[budget_lt(cpu = 1_000, cpu = 2_000)]
fn test_duplicate_metric() {}

fn main() {}
