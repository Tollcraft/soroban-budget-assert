//! Percentage above 100 must also be rejected by the 1–100 range check.
//!
//! The rejection happens while the attribute arguments are parsed, so the
//! annotated function is replaced by the emitted `compile_error!` and never
//! type-checked; its body is therefore empty. The range gate and its 1 and 100
//! boundaries are additionally covered by the `budget_macros::tests` unit
//! tests in `src/lib.rs`.

use budget_macros::budget_cpu_lt;

#[budget_cpu_lt(pct = 101, of = env_file = "tier-a-limits.env", env = "NETWORK__CPU")]
fn test_pct_too_high() {}

fn main() {}