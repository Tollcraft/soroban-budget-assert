//! Duplicate sizes give an expected ratio of 1 for any model, so they are rejected.

use budget_macros::budget_scaling;

#[budget_scaling(sizes = [10, 10], model = linear, tolerance = 0.3)]
fn test_scaling_duplicate_sizes() {}

fn main() {}
