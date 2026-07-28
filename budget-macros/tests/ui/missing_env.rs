use budget_macros::budget_cpu_lt;

// The macro generates code that references `env.cost_estimate().budget()`,
// but no `env` variable exists here — this should fail to compile.
#[budget_cpu_lt(1000)]
fn test_without_env() {
    // No `env` variable — should fail to compile.
}

fn main() {}
