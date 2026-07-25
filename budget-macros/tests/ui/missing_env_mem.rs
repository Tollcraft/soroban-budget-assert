use budget_macros::budget_mem_lt;

// Same as missing_env but exercises budget_mem_lt to confirm both macros
// produce sensible errors when `env` is absent.
#[budget_mem_lt(500000)]
fn test_mem_without_env() {
    // No `env` variable — should fail to compile.
}

fn main() {}
