//! `budget_scaling` must reject a repeated `model` instead of keeping the last.

use budget_macros::budget_scaling;

// error-pattern: `model` cannot be specified more than once

#[budget_scaling(sizes = [10, 100], model = linear, model = quadratic, tolerance = 0.3)]
fn test_duplicate_model() {}

fn main() {}
