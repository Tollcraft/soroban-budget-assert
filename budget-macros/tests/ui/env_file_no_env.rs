//! `env_file` without a matching `env` key: the parser must reject it because
//! it has no variable name to look up in the file.

use budget_macros::budget_cpu_lt;

#[budget_cpu_lt(env_file = "tier-a-limits.env")]
fn test_env_file_no_env() {
    let env = ();
}

fn main() {}
