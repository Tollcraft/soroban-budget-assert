//! Two budget attributes stacked on one function must both expand (#994).
//!
//! Each attribute rewrites the item it sits on and hands the remaining
//! attributes to the next pass, so a stacked pair is the shape a test takes
//! when it bounds CPU and memory at the same time. Nothing in the pass
//! fixtures covered that: every other file writes one attribute per function.
//!
//! The teeth are the two breach cases. If either attribute stopped expanding,
//! the under-limit case would still pass quietly, but the matching breach would
//! return without panicking and `assert_budget_failure` would fail the run.

#[path = "../support/mock_env.rs"]
mod mock_env;

use budget_macros::{budget_cpu_lt, budget_mem_lt};
use mock_env::{budget_panic, Env};

/// Under both limits: the body runs to completion and its value comes back.
#[budget_cpu_lt(4_096)]
#[budget_mem_lt(8_192)]
fn under_both_limits() -> u64 {
    let env = Env::new(900, 1_000);
    env.cost_estimate().budget().cpu_instruction_cost()
}

/// Over the CPU limit only: the memory attribute is still on the function, so
/// a message about memory bytes would mean the two had been interchanged.
#[budget_cpu_lt(4_096)]
#[budget_mem_lt(8_192)]
fn cpu_breach() -> u64 {
    let env = Env::new(9_000, 1_000);
    env.cost_estimate().budget().cpu_instruction_cost()
}

/// Over the memory limit only, which is the same argument read the other way.
#[budget_cpu_lt(4_096)]
#[budget_mem_lt(8_192)]
fn memory_breach() -> u64 {
    let env = Env::new(900, 90_000);
    env.cost_estimate().budget().memory_bytes_cost()
}

fn assert_budget_failure<F, R>(case: &str, expected: &str, f: F)
where
    F: FnOnce() -> R + std::panic::UnwindSafe,
{
    let message = budget_panic(f)
        .unwrap_or_else(|| panic!("{case}: the budget assertion should have failed"));
    assert!(
        message.contains(expected),
        "{case}: unexpected panic message: {message}"
    );
}

fn main() {
    // Both checks ran and neither fired.
    assert_eq!(under_both_limits(), 900);

    assert_budget_failure(
        "cpu breach",
        "CPU instruction cost 9000 exceeded limit 4096",
        cpu_breach,
    );
    assert_budget_failure(
        "memory breach",
        "Memory bytes cost 90000 exceeded limit 8192",
        memory_breach,
    );
}
