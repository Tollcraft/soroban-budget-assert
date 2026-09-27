//! `budget_lt` with an unknown property name.

use budget_macros::budget_lt;

#[budget_lt(unknown_prop = 1000)]
fn test_unknown_property() {
    let env = ();
}

fn main() {}
