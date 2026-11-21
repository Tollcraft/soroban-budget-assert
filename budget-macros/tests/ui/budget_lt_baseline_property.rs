//! `budget_lt` has no plain `baseline`.
//!
//! The single-metric macros (`budget_cpu_lt`, `budget_mem_lt`, …) take
//! `baseline = <expr>`, and the reference page copied that onto `budget_lt`.
//! `BudgetSpec` carries the subtraction per metric instead, so a bare
//! `baseline` is an unknown property — `cpu_baseline` / `mem_baseline` are the
//! keys that exist, and each requires its matching limit.

use budget_macros::budget_lt;

#[budget_lt(cpu = 1_000_000, mem = 1_000_000, baseline = 1_000)]
fn test_plain_baseline_is_not_a_property() {}

fn main() {}
