//! `budget_cpu_lt` takes an integer literal with exactly one optional
//! trailing key: `, baseline = <expr>`. Any other key after the comma — here
//! the plural typo `baselines` — is rejected while the standalone spec is
//! parsed, and the annotated function is replaced by the emitted
//! `compile_error!` before it is ever type-checked, so its body never runs.
//!
//! The diagnostic is pinned below and in `wrong_baseline_key.stderr`; the
//! parser behaviour behind it is additionally covered by the
//! `budget_macros::tests` unit tests in `src/lib.rs`.

use budget_macros::budget_cpu_lt;

#[budget_cpu_lt(1000, baselines = 100)]
fn test_wrong_baseline_key() {}

fn main() {}