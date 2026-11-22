//! A `#[budget_scaling]` attribute carrying no arguments at all must name the
//! missing required field, not expand into an unchecked function.

use budget_macros::budget_scaling;

// Written without parentheses, so the attribute's argument token stream is
// empty — the shape `scaling_missing_sizes.rs` does not reach, because it
// supplies `model` and `tolerance` and only omits `sizes`.
#[budget_scaling]
fn test_no_arguments() {}

fn main() {}
