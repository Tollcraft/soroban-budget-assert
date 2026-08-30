//! Percentage values outside 1–100 must fail at compile time.

use budget_macros::budget_cpu_lt;

#[budget_cpu_lt(pct = 0, of = env_file = "tier-a-limits.env", env = "NETWORK__CPU")]
fn test_pct_zero() {
    let env = ();
}

fn main() {}
