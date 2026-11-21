//! A standalone budget attribute with a baseline but no limit.
//!
//! `StandaloneSpec::parse` (budget-macros/src/lib.rs) parses the budget limit
//! as its initial positional argument before it checks for `baseline = …`, so
//! `#[budget_cpu_lt(baseline = 5)]` never reaches the baseline branch: the
//! limit parser sees the `baseline` identifier where a limit source belongs
//! and rejects it. The diagnostic names the token so the missing limit is
//! obvious, and the caret points at `baseline` rather than at the whole
//! attribute.
//!
//! The exact diagnostic is pinned in `standalone_baseline_only.stderr`.

use budget_macros::budget_cpu_lt;

#[budget_cpu_lt(baseline = 5)]
fn test_baseline_without_limit() {}

fn main() {}
