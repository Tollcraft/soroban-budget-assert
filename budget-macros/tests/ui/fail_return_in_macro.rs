//! A `return` written inside macro invocation tokens cannot be rewritten, so it
//! is rejected instead of silently skipping the budget assertion.

#[path = "support/mock_env.rs"]
mod mock_env;

use budget_macros::budget_cpu_lt;
use mock_env::Env;

#[derive(Debug)]
struct TestError;

#[budget_cpu_lt(1_000)]
fn return_inside_macro(exit_early: bool) -> Result<(), TestError> {
    let env = Env::new(999, 0);
    assert!(if exit_early {
        return Err(TestError)
    } else {
        env.cost_estimate().budget().cpu_instruction_cost() < 1_000
    });
    Ok(())
}

fn main() {
    let _ = return_inside_macro(true);
}
