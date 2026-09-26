//! Unexpected tokens after `baseline = …` in a standalone attribute.
//!
//! `budget_cpu_lt` accepts an integer literal with exactly one optional
//! trailing key, `, baseline = <expr>`. Once that baseline has been consumed
//! the attribute must be at its end — anything still following (here the bogus
//! `, extra = "bad"`) is rejected while the `StandaloneSpec` is parsed, and the
//! annotated function is replaced by the emitted `compile_error!` before it is
//! ever type-checked.
//!
//! The exact diagnostic is pinned in `trailing_tokens_after_baseline.stderr`.

use budget_macros::budget_cpu_lt;

#[budget_cpu_lt(1000, baseline = 100, extra = "bad")]
fn test_trailing_tokens() {}

fn main() {}