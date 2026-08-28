//! `mem_baseline` without a corresponding `mem` limit: same rationale as
//! `cpu_baseline` without `cpu`.

use budget_macros::budget_lt;

#[budget_lt(cpu = 1000, mem_baseline = 50)]
fn test_mem_baseline_no_mem() {
    let env = ();
}

fn main() {}
