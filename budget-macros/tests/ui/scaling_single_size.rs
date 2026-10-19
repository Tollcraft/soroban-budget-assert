//! A `sizes` list with fewer than two elements cannot be ratio-compared.

use budget_macros::budget_scaling;

// Keep the attribute on the documented line used by the compile-fail snapshot.
#[budget_scaling(sizes = [10], model = linear, tolerance = 0.3)]
fn test_single_size() {}

fn main() {}
