//! Baseline budget limits: `baseline = <expr>` and the metric-specific
//! `cpu_baseline` / `mem_baseline` forms subtract a known floor before the
//! measurement is compared with the limit.
//!
//! The macro records the raw measurement first, then evaluates the baseline
//! expression and computes `measurement - baseline` with saturating
//! subtraction. The assertion remains strict (`marginal < limit`), and a
//! baseline at or above the limit therefore still fails. A measurement below
//! the floor is reported as zero rather than wrapping around `u64`.
//!
//! The motivating case is the local WASM instantiation floor: every invocation
//! re-instantiates the module, so a raw measurement is dominated by a constant
//! that the network-derived limits do not include. See `noop` in
//! `amm-pool-contract`.
//!
//! `budget_write_bytes_lt` uses the SDK's memory byte value as its proxy. The
//! combined `budget_lt` form uses separate `cpu_baseline` and `mem_baseline`
//! expressions, while the standalone CPU and memory attributes each take one
//! baseline expression.

#[path = "../support/mock_env.rs"]
mod mock_env;

use budget_macros::{budget_cpu_lt, budget_lt, budget_mem_lt, budget_write_bytes_lt};
use mock_env::{budget_panic, Env};

/// Stand-in for a measured instantiation floor.
///
/// Keeping the value behind a function makes it clear that the macro evaluates
/// a caller-supplied expression, rather than requiring a literal or a
/// pre-computed global.
fn floor() -> u64 {
    1_000
}

/// A raw cost of 1_200 becomes 200 after subtracting the 1_000 floor, so it
/// passes the 500 limit.
#[budget_cpu_lt(500, baseline = floor())]
fn cpu_within_limit_after_baseline() {
    let env = Env::new(1_200, 0);
}

/// A raw cost of 1_900 becomes 900, which still exceeds the 500 limit; a
/// baseline changes the comparison rather than disabling it.
#[budget_cpu_lt(500, baseline = floor())]
fn cpu_over_limit_after_baseline() {
    let env = Env::new(1_900, 0);
}

/// The strict boundary is inclusive on the failing side: marginal cost 500
/// must fail a limit of 500.
#[budget_cpu_lt(500, baseline = floor())]
fn cpu_at_limit_after_baseline_panics() {
    let env = Env::new(1_500, 0);
}

/// Memory uses the same baseline subtraction and strict comparison rules.
#[budget_mem_lt(500, baseline = floor())]
fn mem_within_limit_after_baseline() {
    let env = Env::new(0, 1_200);
}

/// Write-byte checks use the memory byte value as the metric proxy.
#[budget_write_bytes_lt(500, baseline = floor())]
fn write_bytes_within_limit_after_baseline() {
    let env = Env::new(0, 1_200);
}

/// Combined checks apply the independent CPU and memory floors to their own
/// metrics before enforcing their respective limits.
#[budget_lt(
    cpu = 500,
    mem = 500,
    cpu_baseline = floor(),
    mem_baseline = floor()
)]
fn both_within_limit_after_baseline() {
    let env = Env::new(1_200, 1_400);
}

/// A measurement below the baseline saturates to zero rather than wrapping
/// around `u64` and spuriously failing.
#[budget_cpu_lt(500, baseline = floor())]
fn cpu_below_baseline_saturates() {
    let env = Env::new(10, 0);
}

/// A sourced limit can be combined with a baseline. An unset environment
/// variable resolves to `u64::MAX`, so this case passes without relying on the
/// process environment.
#[budget_cpu_lt(env = "PASS_BASELINE_UNSET_LIMIT", baseline = floor())]
fn cpu_baseline_with_env_limit() {
    let env = Env::new(1_200, 0);
}

/// Cases whose marginal cost lands below the limit, including the saturating
/// and env-sourced variants.
fn run_passing_cases() {
    cpu_within_limit_after_baseline();
    mem_within_limit_after_baseline();
    write_bytes_within_limit_after_baseline();
    both_within_limit_after_baseline();
    cpu_below_baseline_saturates();
    cpu_baseline_with_env_limit();
}

/// Cases whose marginal cost lands at or above the limit. Each assertion
/// pins both the asserted figure and the `measured - baseline` breakdown so a
/// regression in the arithmetic cannot hide behind an unchanged headline
/// number.
fn run_failing_cases() {
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

    let boundary_message = budget_panic(cpu_at_limit_after_baseline_panics)
        .expect("the exact marginal boundary should fail");
    assert!(
        boundary_message.contains("CPU instruction cost 500 exceeded limit 500"),
        "the exact marginal boundary should be reported: {boundary_message}"
    );
    assert!(
        boundary_message.contains("1500 measured - 1000 baseline"),
        "the boundary message should show the raw values: {boundary_message}"
    );
}

fn main() {
    run_passing_cases();
    run_failing_cases();
}
