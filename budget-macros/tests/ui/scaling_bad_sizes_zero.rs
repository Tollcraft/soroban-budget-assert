//! A zero in `sizes` makes the expected ratio infinite, so it is rejected.

use budget_macros::budget_scaling;

#[budget_scaling(sizes = [0, 10], model = linear, tolerance = 0.3)]
fn test_scaling_zero_size() {}

fn main() {}
