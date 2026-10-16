//! Genuine dead code after a `return` must still be reported: the macro only
//! suppresses its own trailing assertion, not the whole function.

#[path = "support/mock_env.rs"]
mod mock_env;

use budget_macros::budget_cpu_lt;
use mock_env::Env;

#[deny(unreachable_code)]
#[budget_cpu_lt(1_000_000)]
fn dead_code_after_return() {
    let env = Env::new(10, 0);
    let _ = env.cost_estimate().budget().cpu_instruction_cost();
    return;
    let _unreachable = 1u32;
}

fn main() {}
