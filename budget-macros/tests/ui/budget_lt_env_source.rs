//! A bare `env = "…"` names a limit source, not a `budget_lt` property.

use budget_macros::budget_lt;

#[budget_lt(env = "MAX_CPU_INSTRUCTIONS")]
fn test_spec_level_env_source() {}

fn main() {}
