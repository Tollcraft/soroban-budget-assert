use budget_macros::budget_cpu_lt;

// error-pattern: `config` cannot be specified more than once

#[budget_cpu_lt(config = "first", config = "second")]
fn test_duplicate_config() {}

fn main() {}
