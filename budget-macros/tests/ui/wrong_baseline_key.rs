//! `budget_cpu_lt` with a comma followed by a non-`baseline` key.

use budget_macros::budget_cpu_lt;

#[budget_cpu_lt(1000, baselines = 100)]
fn test_wrong_baseline_key() {
    let env = ();
}

fn main() {}
