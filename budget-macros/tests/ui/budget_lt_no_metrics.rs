//! `budget_lt` requires at least one of `cpu` or `mem` to be specified.

use budget_macros::budget_lt;

#[budget_lt]
fn test_no_metrics() {
    let env = ();
}

fn main() {}
