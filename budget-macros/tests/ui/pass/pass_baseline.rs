//! `baseline = <expr>` / `cpu_baseline` + `mem_baseline` subtract a fixed floor
//! from the measurement before it is compared against the limit.
//!
//! The motivating case is the local WASM instantiation floor: every invocation
//! re-instantiates the module, so a raw measurement is dominated by a constant
//! that the network-derived limits do not include. See `noop` in
//! `amm-pool-contract`.

#[path = "../support/mock_env.rs"]
mod mock_env;

use budget_macros::{budget_cpu_lt, budget_lt, budget_mem_lt, budget_write_bytes_lt};
use mock_env::{budget_panic, Env};

/// Stands in for a measured instantiation floor.
fn floor() -> u64 {
    1_000
}

// 1_200 measured - 1_000 baseline = 200 marginal, under the 500 limit.
// Without the baseline the raw 1_200 would blow through it.
#[budget_cpu_lt(500, baseline = floor())]
fn cpu_within_limit_after_baseline() {
    let env = Env::new(1_200, 0);
}

// 1_900 - 1_000 = 900 marginal, over the 500 limit: a baseline shifts the
// comparison, it does not disable it.
#[budget_cpu_lt(500, baseline = floor())]
fn cpu_over_limit_after_baseline() {
    let env = Env::new(1_900, 0);
}

#[budget_mem_lt(500, baseline = floor())]
fn mem_within_limit_after_baseline() {
    let env = Env::new(0, 1_200);
}

// `budget_write_bytes_lt` measures memory bytes as its proxy.
#[budget_write_bytes_lt(500, baseline = floor())]
fn write_bytes_within_limit_after_baseline() {
    let env = Env::new(0, 1_200);
}

// Both metrics at once, each with its own baseline.
#[budget_lt(
    cpu = 500,
    mem = 500,
    cpu_baseline = floor(),
    mem_baseline = floor()
)]
fn both_within_limit_after_baseline() {
    let env = Env::new(1_200, 1_400);
}

// A measurement below the baseline saturates to 0 rather than wrapping around
// u64 and spuriously failing.
#[budget_cpu_lt(500, baseline = floor())]
fn cpu_below_baseline_saturates() {
    let env = Env::new(10, 0);
}

// The limit may still come from any of the usual sources.
#[budget_cpu_lt(env = "PASS_BASELINE_UNSET_LIMIT", baseline = floor())]
fn cpu_baseline_with_env_limit() {
    let env = Env::new(1_200, 0);
}

fn main() {
    cpu_within_limit_after_baseline();
    mem_within_limit_after_baseline();
    write_bytes_within_limit_after_baseline();
    both_within_limit_after_baseline();
    cpu_below_baseline_saturates();
    // Unset env var means "no limit" (u64::MAX), so this passes.
    cpu_baseline_with_env_limit();

    let message = budget_panic(cpu_over_limit_after_baseline)
        .expect("the budget assertion should have failed");
    assert!(
        message.contains("CPU instruction cost 900 exceeded limit 500"),
        "marginal cost should be the asserted figure: {message}"
    );
    assert!(
        message.contains("1900 measured - 1000 baseline"),
        "the message should show the raw measurement and the baseline: {message}"
    );
}
