//! An env file path that does not exist must be rejected during expansion.

use budget_macros::budget_cpu_lt;

// Keep the attribute on the documented line used by the compile-fail snapshot.
#[budget_cpu_lt(env_file = "definitely/not/a/real/limits.env", env = "SOME_LIMIT")]
fn test_env_file_missing_file() {}

fn main() {}
