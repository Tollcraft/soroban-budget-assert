use budget_macros::budget_cpu_lt;

#[budget_cpu_lt(env_file = "a.env", env_file = "b.env", env = "ONE")]
fn test_duplicate_env_file() {}

fn main() {}
