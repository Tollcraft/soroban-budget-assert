//! `env` combined with `config`: both provide an absolute value from different
//! sources, so only one is allowed.

use budget_macros::budget_cpu_lt;

#[budget_cpu_lt(env = "CPU_LIMIT", config = "cpu_limit")]
fn test_env_with_config() {
    let env = ();
}

fn main() {}
