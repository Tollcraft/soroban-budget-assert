//! `env_file` without a matching `env` key: the parser must reject it because
//! it has no variable name to look up in the file.
//! This ensures developers specify an `env` key when utilizing an `env_file` configuration.

use budget_macros::budget_cpu_lt;

#[budget_cpu_lt(env_file = "tier-a-limits.env")]
fn test_env_file_no_env() {
    // Explicitly binding without using to prevent unused variable warnings in UI tests.
    let _env = ();
}

fn main() {}
