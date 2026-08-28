//! Integer literal combined with `pct` must fail: an absolute literal and a
//! percentage source are contradictory.

use budget_macros::budget_cpu_lt;

#[budget_cpu_lt(1000, pct = 25, of = env_file = "tier-a-limits.env", env = "NETWORK__CPU")]
fn test_int_and_pct() {
    let env = ();
}

fn main() {}
