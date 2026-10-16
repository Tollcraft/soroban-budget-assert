//! `cpu = pct = N` inside `#[budget_lt(...)]` still requires `of = <source>`.

use budget_macros::budget_lt;

#[budget_lt(cpu = pct = 50)]
fn test_budget_lt_pct_no_of() {
    let env = ();
}

fn main() {}
