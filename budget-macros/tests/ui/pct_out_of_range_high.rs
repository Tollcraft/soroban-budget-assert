//! Percentage above 100 must also be rejected by the 1–100 range check.

use budget_macros::budget_cpu_lt;

#[budget_cpu_lt(pct = 101, of = env_file = "tier-a-limits.env", env = "NETWORK__CPU")]
fn test_pct_too_high() {
    let env = ();
}

fn main() {}
