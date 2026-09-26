//! `env_file` combined with `config`: both name an absolute limit, but from
//! different sources, so only one of them may be supplied.
//!
//! The rejection happens while the attribute arguments are parsed, so the
//! annotated function is replaced by the emitted `compile_error!` and never
//! runs. The body is intentionally empty: it exists only to give the attribute
//! something to annotate, and the previous `let env = ();` placeholder was
//! dead weight that could never execute.

use budget_macros::budget_cpu_lt;

#[budget_cpu_lt(env_file = "tier-a-limits.env", config = "cpu_limit")]
fn test_env_file_with_config() {}

fn main() {}