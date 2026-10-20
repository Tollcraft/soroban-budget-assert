//! Pass fixture: since #952 a malformed env-file line is skipped instead of
//! aborting the lookup, so the valid `TARGET_CPU_LIMIT` line still resolves.

#[allow(unused_imports)]
use budget_macros::budget_cpu_lt;

#[path = "../support/mock_env.rs"]
mod mock_env;

#[test]
#[budget_cpu_lt(env_file = "tests/ui/env_file_malformed.env", env = "TARGET_CPU_LIMIT")]
fn test_env_file_malformed_lines_skipped() {
    let env = mock_env::Env::new(100_000, 100_000);
    assert!(env.cost_estimate().budget().cpu_instruction_cost() < 1_250_000);
}

fn main() {}
