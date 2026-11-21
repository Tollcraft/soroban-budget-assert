//! The reference page's "Limit from `.env` file" example for `budget_lt`,
//! verbatim, as a top-level argument list.
//!
//! A limit *source* is not a `budget_lt` property: `env_file` / `env` have to
//! be nested under the metric they bound —
//! `cpu = env_file = "...", env = "..."` — otherwise the spec parser rejects
//! the key. This is the line `docs/src/reference.md` still shows for #806; the
//! page now presents it as a rejected form, and this fixture is the diagnostic
//! that makes that true.

use budget_macros::budget_lt;

#[budget_lt(env_file = "../tier-a-limits.env", env = "TIER_A__AMM_POOL_CONTRACT__SWAP__CPU")]
fn test_env_file_source_at_top_level() {}

fn main() {}
