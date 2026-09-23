//! Integer literal combined with `pct` must fail: an absolute literal and a
//! percentage source both name the same value, so combining them is
//! contradictory.
//!
//! The attribute is rejected while its arguments are parsed, so the annotated
//! function is replaced by the emitted `compile_error!` before it is ever
//! type-checked. Its body therefore needs no `env` binding and carries none —
//! a `let env = ();` placeholder would be dead code that never runs, and only
//! obscures what this fixture actually pins (the parser diagnostic below).
//!
//! The matching `.stderr` snapshot pins that diagnostic; the parser behaviour
//! behind it is also covered by the `budget_macros::tests` unit tests in
//! `src/lib.rs`.

use budget_macros::budget_cpu_lt;

#[budget_cpu_lt(1000, pct = 25, of = env_file = "tier-a-limits.env", env = "NETWORK__CPU")]
fn test_int_and_pct() {}

fn main() {}
