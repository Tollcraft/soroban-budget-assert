//! A budget attribute on an `impl` block instruments every method in it.
//!
//! Covered here:
//!   * block-level application, including associated items and `self` receivers;
//!   * strict limit boundaries and failures on both normal and early-return paths;
//!   * block baselines and method attribution in their diagnostics;
//!   * per-method overrides, including a different budget metric;
//!   * explicit no-limit methods, trait implementations, and trailing results.

#[path = "../support/mock_env.rs"]
mod mock_env;

use budget_macros::{budget_cpu_lt, budget_mem_lt};
use mock_env::{budget_panic, Env};

#[derive(Debug, PartialEq)]
struct TestError;

struct Contract;
struct MemoryContract;
struct BaselineContract;
struct StatefulContract {
    calls: u32,
}

trait Worker {
    fn work(&self) -> u64;
}

struct WorkerContract;

fn floor() -> u64 {
    500
}

#[budget_cpu_lt(1_000)]
impl Contract {
    const BLOCK_LIMIT: u64 = 1_000;

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

    // A method may deliberately opt out by shadowing the injected resolver so
    // it returns no limit. The cost is one below u64::MAX to pin the strict
    // comparison without relying on inherited process environment state.
    #[budget_cpu_lt(env = "PASS_IMPL_BLOCK_UNSET_LIMIT")]
    fn deliberately_unbudgeted() {
        let budget_env_resolve = |_var: &str| -> Option<String> { None };
        let env = Env::new(u64::MAX - 1, 0);
        let _ = env.cost_estimate().budget().cpu_instruction_cost();
    }

    #[budget_cpu_lt(4_000)]
    fn at_its_own_override_limit() {
        let env = Env::new(4_000, 0);
    }

    fn result_body() -> Result<u64, TestError> {
        let env = Env::new(900, 0);
        Ok(env.cost_estimate().budget().cpu_instruction_cost())
    }

    fn at_block_limit() {
        let env = Env::new(1_000, 0);
    }

    fn over_the_block_limit() {
        let env = Env::new(2_500, 0);
    }

    fn over_limit_on_early_return(exit_early: bool) -> Result<(), TestError> {
        let env = Env::new(1_500, 0);
        if exit_early {
            return Ok(());
        }
        Ok(())
    }
}

#[budget_cpu_lt(1_000)]
impl MemoryContract {
    fn block_limit_still_applies() {
        let env = Env::new(900, 0);
    }

    // A method-level attribute for another metric prevents the block-level CPU
    // expansion. The deliberately maximal CPU cost proves the method uses only
    // its own memory limit.
    #[budget_mem_lt(4_000)]
    fn method_override_controls_measurement() {
        let env = Env::new(u64::MAX, 3_999);
    }
}

#[budget_cpu_lt(1_000, baseline = floor())]
impl BaselineContract {
    fn within_limit_after_baseline() {
        let env = Env::new(1_200, 0);
    }

    fn over_limit_after_baseline() {
        let env = Env::new(1_600, 0);
    }
}

#[budget_cpu_lt(1_000)]
impl StatefulContract {
    fn record_call(&mut self) {
        let env = Env::new(500, 0);
        self.calls += 1;
    }
}

#[budget_cpu_lt(1_000)]
impl Worker for WorkerContract {
    fn work(&self) -> u64 {
        let env = Env::new(800, 0);
        env.cost_estimate().budget().cpu_instruction_cost()
    }
}

fn assert_block_failure<F, R>(case: &str, cost: u64, limit: u64, method: &str, f: F)
where
    F: FnOnce() -> R + std::panic::UnwindSafe,
{
    let message = budget_panic(f)
        .unwrap_or_else(|| panic!("{case}: the budget assertion should have failed"));
    assert!(
        message.contains(&format!(
            "CPU instruction cost {cost} exceeded limit {limit}"
        )),
        "{case}: unexpected panic message: {message}"
    );
    assert!(
        message.contains(&format!("[fn `{method}`]")),
        "{case}: the failure must name `{method}`: {message}"
    );
}

fn main() {
    assert_eq!(Contract::BLOCK_LIMIT, 1_000);
    Contract::cheap_entrypoint();
    Contract::also_cheap();
    Contract::expensive_but_within_its_own_limit();
    Contract::deliberately_unbudgeted();
    assert_eq!(Contract::result_body(), Ok(900));

    let override_message = budget_panic(Contract::at_its_own_override_limit)
        .expect("the method-level limit should be strict");
    assert!(
        override_message.contains("CPU instruction cost 4000 exceeded limit 4000"),
        "the method's own limit should be enforced: {override_message}"
    );

    MemoryContract::block_limit_still_applies();
    MemoryContract::method_override_controls_measurement();

    BaselineContract::within_limit_after_baseline();
    let baseline_message = budget_panic(BaselineContract::over_limit_after_baseline)
        .expect("the block baseline should still enforce the marginal limit");
    assert!(
        baseline_message.contains("CPU instruction cost 1100 exceeded limit 1000"),
        "unexpected baseline panic message: {baseline_message}"
    );
    assert!(
        baseline_message.contains("1600 measured - 500 baseline"),
        "the baseline details should be present: {baseline_message}"
    );
    assert!(
        baseline_message.contains("[fn `over_limit_after_baseline`]"),
        "the baseline failure should name the method: {baseline_message}"
    );

    let mut state = StatefulContract { calls: 0 };
    state.record_call();
    assert_eq!(state.calls, 1);
    assert_eq!(WorkerContract.work(), 800);

    assert_block_failure(
        "strict boundary",
        1_000,
        1_000,
        "at_block_limit",
        Contract::at_block_limit,
    );
    assert_block_failure(
        "normal failure",
        2_500,
        1_000,
        "over_the_block_limit",
        Contract::over_the_block_limit,
    );
    assert_block_failure(
        "early return failure",
        1_500,
        1_000,
        "over_limit_on_early_return",
        || Contract::over_limit_on_early_return(true),
    );
}
