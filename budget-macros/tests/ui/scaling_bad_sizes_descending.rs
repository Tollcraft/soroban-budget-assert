//! Sizes must ascend so consecutive ratios are well-defined.

use budget_macros::budget_scaling;

#[budget_scaling(sizes = [100, 10], model = linear, tolerance = 0.3)]
fn test_scaling_descending_sizes() {}

fn main() {}
