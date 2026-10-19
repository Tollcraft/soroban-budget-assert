//! Scaling configuration must reject keys outside `sizes`, `model`, `tolerance`.

use budget_macros::budget_scaling;

// Keep the attribute on the documented line used by the compile-fail snapshot.
#[budget_scaling(sizes = [10, 100], model = linear, tolerance = 0.3, steps = 4)]
fn test_unknown_scaling_property() {}

fn main() {}
