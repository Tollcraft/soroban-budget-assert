//! `env_file` combined with `config`: these are two different sources for the
//! limit and cannot coexist.

use budget_macros::budget_cpu_lt;

#[budget_cpu_lt(env_file = "tier-a-limits.env", config = "cpu_limit")]
fn test_env_file_with_config() {
    let env = ();
}

fn main() {}
