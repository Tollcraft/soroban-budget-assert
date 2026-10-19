use budget_macros::budget_cpu_lt;

#[budget_cpu_lt(env = "ONE", env = "TWO")]
fn test_duplicate_env() {}

fn main() {}
