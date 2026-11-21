//! `env_ident` reaching every method of a budget-annotated `impl` block
//! (issue #996).
//!
//! `budget_lt` passes its spec to `generate_budget_assert`, which honours an
//! explicit `env_ident` and defaults to `env`. A block-level `budget_lt`
//! expands per method through `expand_targets`, so the chosen identifier has
//! to survive that per-method hand-off: an ident resolved once at the block
//! and re-emitted as the default `env` for the bodies would compile against
//! `pass_impl_block.rs`, which binds `env` everywhere, and would break every
//! contract that names its environment something else.
//!
//! Both metrics are covered: the CPU-only block proves the ident reaches the
//! single emitted check, and the CPU-plus-memory block proves one ident feeds
//! both metric reads in the same expansion.
//!
//! Removing `env_ident = e` from either attribute makes this fixture fail to
//! compile — that is the contract, and `tests/ui/missing_env.rs` pins the
//! diagnostic for the bare-function case.

#[path = "../support/mock_env.rs"]
mod mock_env;

use budget_macros::budget_lt;
use mock_env::{budget_panic, Env};

struct RenamedEnvContract;
struct BothMetricsContract;

/// Every method binds its environment as `e`, and none binds `env`.
#[budget_lt(cpu = 1_000, env_ident = e)]
impl RenamedEnvContract {
    /// Under the block limit: the check runs against `e` and returns.
    fn within_block_limit() {
        let e = Env::new(900, 0);
        let _ = e.cost_estimate().budget().cpu_instruction_cost();
    }

    /// Over the block limit: the check still runs, against `e`, and panics.
    fn over_block_limit() {
        let e = Env::new(1_500, 0);
    }

    /// A trailing expression is evaluated before the injected check, and the
    /// check reads `e` afterwards.
    fn returns_after_the_check() -> u64 {
        let e = Env::new(800, 0);
        e.cost_estimate().budget().cpu_instruction_cost()
    }
}

/// One `env_ident` has to serve both the CPU and the memory read.
#[budget_lt(cpu = 1_000, mem = 1_000, env_ident = e)]
impl BothMetricsContract {
    /// Both metrics under their limits.
    fn both_within_limit() {
        let e = Env::new(500, 600);
    }

    /// Memory breaches while CPU is comfortably below its limit.
    fn over_memory_limit() {
        let e = Env::new(500, 2_000);
    }
}

/// A block failure must name its method and report the metric that breached.
fn assert_env_ident_failure<F, R>(label: &str, expected: &str, f: F)
where
    F: FnOnce() -> R + std::panic::UnwindSafe,
{
    let message = budget_panic(f)
        .unwrap_or_else(|| panic!("`{label}` should have failed its budget check"));
    assert!(
        message.contains(expected),
        "`{label}`: unexpected panic message: {message}"
    );
    assert!(
        message.contains(&format!("[fn `{label}`]")),
        "`{label}`: the failure must name the method it came from: {message}"
    );
}

fn main() {
    RenamedEnvContract::within_block_limit();
    assert_eq!(RenamedEnvContract::returns_after_the_check(), 800);
    assert_env_ident_failure(
        "over_block_limit",
        "CPU instruction cost 1500 exceeded limit 1000",
        RenamedEnvContract::over_block_limit,
    );

    BothMetricsContract::both_within_limit();
    let message = budget_panic(BothMetricsContract::over_memory_limit)
        .expect("the memory limit should be enforced through `e`");
    assert!(
        message.contains("Memory bytes cost 2000 exceeded limit 1000"),
        "the memory read must go through the renamed env ident: {message}"
    );
    assert!(
        !message.contains("CPU instruction cost"),
        "CPU is under its limit, so only the memory metric may be reported: {message}"
    );
    assert!(
        message.contains("[fn `over_memory_limit`]"),
        "the failure must name the method: {message}"
    );
}
