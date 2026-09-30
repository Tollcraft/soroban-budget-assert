//! Scaling configuration must use one of the supported growth models.

use budget_macros::budget_scaling;

// Keep the attribute on the documented line used by the compile-fail snapshot.
#[budget_scaling(sizes = [10, 100], model = exponential, tolerance = 0.3)]
fn test_unknown_growth_model() {}

fn main() {}
