//! Missing comma between a numeric limit and `baseline`.
//!
//! `StandaloneSpec::parse` (budget-macros/src/lib.rs) requires a `Token![,]`
//! between the positional limit and any following `baseline = …`, so
//! `#[budget_cpu_lt(1000 baseline = 5)]` is not valid syntax. The diagnostic
//! must place the caret on the offending `baseline` identifier rather than
//! spanning the whole attribute — a caret that covers `#[budget_cpu_lt(...)]`
//! would hide the fix at a glance.

use budget_macros::budget_cpu_lt;

#[budget_cpu_lt(1000 baseline = 5)]
fn test_missing_comma_after_limit() {}

fn main() {}
