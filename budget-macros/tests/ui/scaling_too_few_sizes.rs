//! Ratio comparison needs at least two sizes; the diagnostic points at the
//! sizes list.

use budget_macros::budget_scaling;

#[budget_scaling(sizes = [10], model = linear, tolerance = 0.3)]
fn test_scaling_single_size() {}

fn main() {}
