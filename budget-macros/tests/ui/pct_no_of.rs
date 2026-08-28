//! `pct` without `of` must fail: the percentage needs a reference limit.

use budget_macros::budget_cpu_lt;

#[budget_cpu_lt(pct = 25)]
fn test_pct_no_of() {
    let env = ();
}

fn main() {}
