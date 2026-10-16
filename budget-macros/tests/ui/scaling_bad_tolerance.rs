//! A negative tolerance would fail every scaling check for the wrong reason.

use budget_macros::budget_scaling;

#[budget_scaling(sizes = [10, 100], model = linear, tolerance = -0.5)]
fn test_scaling_negative_tolerance() {}

fn main() {}
