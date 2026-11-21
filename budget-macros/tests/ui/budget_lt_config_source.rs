//! The reference page's "Config-driven limit" example for `budget_lt`.
//!
//! `config` is a limit source like `env`, so the same rule applies: it is only
//! valid nested inside `cpu =` / `mem =`, and as a top-level property the spec
//! parser names it and shows the nesting that is wanted.

use budget_macros::budget_lt;

#[budget_lt(config = "generic_budget")]
fn test_config_source_at_top_level() {}

fn main() {}
