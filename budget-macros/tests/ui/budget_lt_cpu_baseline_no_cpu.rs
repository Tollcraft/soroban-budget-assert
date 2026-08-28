//! `cpu_baseline` without a corresponding `cpu` limit: the baseline would be
//! subtracted from nothing, silently doing nothing.

use budget_macros::budget_lt;

#[budget_lt(cpu_baseline = 100, mem = 500)]
fn test_cpu_baseline_no_cpu() {
    let env = ();
}

fn main() {}
