//! A percentage memory limit must resolve against the network reference and
//! say so in the failure text.
//!
//! `budget_mem_lt` is its own attribute entry point, separate from
//! `budget_cpu_lt`, and it reaches the shared message builder with the
//! `Memory bytes cost` phrase. Nothing pinned the memory side of percentage
//! formatting: `pass_percentage_limit.rs` covers CPU and asserts only the
//! loose `{pct}%` substring, `pass_budget_lt.rs` pins the exact suffix for CPU,
//! `pass_events.rs` covers events, and `pass_nested_pct.rs` combines a CPU
//! percentage with a *literal* `mem = 500` sibling, so the memory message it
//! asserts has no suffix at all. The `(N% of network limit)` suffix is produced
//! per-limit inside `assert_messages`, so a regression that dropped it for
//! memory assertions — or resolved the limit against the wrong reference — had
//! no fixture left.
//!
//! The reference is read from the fixture env file at test runtime, so the
//! arithmetic is integer division with truncation: `reference * pct / 100`.
//! `PCT_TEST_MEM_NETWORK` is `20_000`, which makes 25% resolve to 5_000, 50% to
//! 10_000 and 100% to 20_000; `PCT_TEST_SMALL_NETWORK` is `50`, so 1% of it
//! resolves to 0 and the strict comparison fails on any non-zero cost.
//!
//! Both message shapes are exercised, because the suffix is emitted twice with
//! different punctuation: alone in the plain message, and after the baseline
//! arithmetic in the marginal one.

#[path = "../support/mock_env.rs"]
mod mock_env;

use budget_macros::budget_mem_lt;
use mock_env::{budget_panic, Env};

/// Network limits live in the checked-in fixture file that the `of = env_file`
/// source reads at test runtime.
const PCT_ENV_FILE: &str = "tests/ui/support/pass_env_file.env";

// 25% of the 20_000-byte memory reference resolves to 5_000. The comparison is
// strict, so 4_999 is the largest passing cost.
#[budget_mem_lt(pct = 25, of = env_file = PCT_ENV_FILE, env = "PCT_TEST_MEM_NETWORK")]
fn quarter_limit_passes_below() {
    let env = Env::new(0, 4_999);
}

/// A cost above the resolved limit fails, and the message must show the
/// resolved figure rather than the 20_000 reference.
#[budget_mem_lt(pct = 25, of = env_file = PCT_ENV_FILE, env = "PCT_TEST_MEM_NETWORK")]
fn quarter_limit_panics_above() {
    let env = Env::new(0, 5_001);
}

/// Equality with the resolved limit is a failure, not a pass.
#[budget_mem_lt(pct = 25, of = env_file = PCT_ENV_FILE, env = "PCT_TEST_MEM_NETWORK")]
fn quarter_limit_panics_at_boundary() {
    let env = Env::new(0, 5_000);
}

/// 50% of the same reference is 10_000.
#[budget_mem_lt(pct = 50, of = env_file = PCT_ENV_FILE, env = "PCT_TEST_MEM_NETWORK")]
fn half_limit_passes_below() {
    let env = Env::new(0, 9_999);
}

/// 100% is the whole reference: 20_000, so a cost just under it passes.
#[budget_mem_lt(pct = 100, of = env_file = PCT_ENV_FILE, env = "PCT_TEST_MEM_NETWORK")]
fn full_network_limit_passes_below() {
    let env = Env::new(0, 19_999);
}

/// Truncating integer arithmetic resolves 1% of the 50-byte small reference to
/// zero, so the assertion fails on the smallest possible cost.
#[budget_mem_lt(pct = 1, of = env_file = PCT_ENV_FILE, env = "PCT_TEST_SMALL_NETWORK")]
fn zero_resolved_limit_panics() {
    let env = Env::new(0, 1);
}

/// Percentage and baseline together: 11_999 measured - 1_000 baseline = 10_999
/// marginal, above the 10_000 that 50% resolves to.
#[budget_mem_lt(
    pct = 50,
    of = env_file = PCT_ENV_FILE,
    env = "PCT_TEST_MEM_NETWORK",
    baseline = 1_000
)]
fn percentage_with_baseline_panics() {
    let env = Env::new(0, 11_999);
}

/// The same pair one byte of marginal cost lower passes, so the failure above
/// is the marginal comparison and not the percentage alone.
#[budget_mem_lt(
    pct = 50,
    of = env_file = PCT_ENV_FILE,
    env = "PCT_TEST_MEM_NETWORK",
    baseline = 1_000
)]
fn percentage_with_baseline_passes_just_below() {
    let env = Env::new(0, 10_999);
}

/// CPU cost is a different metric, so a body over the network CPU figure stays
/// inside a memory percentage limit.
#[budget_mem_lt(pct = 25, of = env_file = PCT_ENV_FILE, env = "PCT_TEST_MEM_NETWORK")]
fn cpu_cost_does_not_use_memory_limit() {
    let env = Env::new(u64::MAX, 4_000);
}

fn assert_memory_failure<F, R>(case: &str, expected: &str, f: F)
where
    F: FnOnce() -> R + std::panic::UnwindSafe,
{
    let message = budget_panic(f)
        .unwrap_or_else(|| panic!("{case}: the memory budget assertion should have failed"));
    assert!(
        message.contains(expected),
        "{case}: unexpected panic message: {message}"
    );
}

fn main() {
    quarter_limit_passes_below();
    half_limit_passes_below();
    full_network_limit_passes_below();
    percentage_with_baseline_passes_just_below();
    cpu_cost_does_not_use_memory_limit();

    // The resolved limit and the percentage both appear; the suffix is the part
    // a CPU-only test could never notice going missing here.
    assert_memory_failure(
        "over the quarter limit",
        "Memory bytes cost 5001 exceeded limit 5000 (25% of network limit)",
        quarter_limit_panics_above,
    );
    assert_memory_failure(
        "at the quarter limit",
        "Memory bytes cost 5000 exceeded limit 5000 (25% of network limit)",
        quarter_limit_panics_at_boundary,
    );
    assert_memory_failure(
        "a percentage that truncates to zero",
        "Memory bytes cost 1 exceeded limit 0 (1% of network limit)",
        zero_resolved_limit_panics,
    );
    // The marginal message keeps the suffix too, after the baseline figures.
    assert_memory_failure(
        "percentage with baseline",
        "Memory bytes cost 10999 exceeded limit 10000 \
         (marginal: 11999 measured - 1000 baseline, 50% of network limit)",
        percentage_with_baseline_panics,
    );
}
