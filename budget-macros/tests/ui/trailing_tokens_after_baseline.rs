//! Unexpected tokens after `baseline = …` in a standalone attribute.

use budget_macros::budget_cpu_lt;

#[budget_cpu_lt(1000, baseline = 100, extra = "bad")]
fn test_trailing_tokens() {
    let env = ();
}

fn main() {}
