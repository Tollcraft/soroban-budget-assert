//! `budget_scaling` with an unrecognized growth model: the parser must reject
//! unknown model names at compile time.

use budget_macros::budget_scaling;

#[budget_scaling(sizes = [10, 100], model = exponential, tolerance = 0.3)]
fn test_unknown_model() {
    // Body is irrelevant — the attribute parser rejects the model before the
    // body is emitted.
}

fn main() {}
