//! A budget attribute on an `impl` block instruments every method in it.
//!
//! Covered here:
//!   * block-level application — plain methods inherit the block's limit;
//!   * per-method override — a method with its own `#[budget_*]` attribute is
//!     governed by that, not the block limit;
//!   * opt-out — a method that should not be asserted carries its own
//!     attribute with an unset `env` limit (which resolves to "no limit");
//!   * attribution — a block-limit failure names the offending method.

#[path = "../support/mock_env.rs"]
mod mock_env;

use budget_macros::budget_cpu_lt;
use mock_env::{budget_panic, Env};

struct Contract;

#[budget_cpu_lt(1_000)]
impl Contract {
    fn cheap_entrypoint() {
        let env = Env::new(500, 0);
        let _ = env.cost_estimate().budget().cpu_instruction_cost();
    }

    fn also_cheap() {
        let env = Env::new(900, 0);
        let _ = env.cost_estimate().budget().cpu_instruction_cost();
    }

    // Per-method override: 3_000 is over the block's 1_000 but under this
    // method's own 4_000, so it passes.
    #[budget_cpu_lt(4_000)]
    fn expensive_but_within_its_own_limit() {
        let env = Env::new(3_000, 0);
        let _ = env.cost_estimate().budget().cpu_instruction_cost();
    }

    // Opt-out: an unset env limit resolves to u64::MAX, so this method is
    // effectively not asserted even though it sits in the annotated block.
    #[budget_cpu_lt(env = "PASS_IMPL_BLOCK_UNSET_LIMIT")]
    fn deliberately_unbudgeted() {
        let env = Env::new(9_999, 0);
        let _ = env.cost_estimate().budget().cpu_instruction_cost();
    }

    fn over_the_block_limit() {
        let env = Env::new(2_500, 0);
        let _ = env.cost_estimate().budget().cpu_instruction_cost();
    }
}

fn main() {
    Contract::cheap_entrypoint();
    Contract::also_cheap();
    Contract::expensive_but_within_its_own_limit();
    Contract::deliberately_unbudgeted();

    let message = budget_panic(Contract::over_the_block_limit)
        .expect("the block-level limit should have failed this method");
    assert!(
        message.contains("CPU instruction cost 2500 exceeded limit 1000"),
        "unexpected panic message: {message}"
    );
    assert!(
        message.contains("[fn `over_the_block_limit`]"),
        "the failure must name the offending method: {message}"
    );
}
