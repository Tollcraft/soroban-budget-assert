//! One `#[budget_lt]` whose two metrics get their limits from *different*
//! sources (issue #995).
//!
//! CPU is a percentage of a network limit read out of the fixture env file,
//! memory is that env file's value used directly as the limit, and each metric
//! carries its own baseline. `BudgetLimit::parse` accepts a literal, `env`,
//! `env_file`, `config` and `pct`, and `BudgetSpec::parse` takes one source per
//! metric, so the combination is legal but was never exercised: `pass_baseline`
//! pins baselines against literal limits, `pass_env_file` pins the env-file
//! source alone, and `pass_nested_pct` pins a percentage next to a literal
//! sibling. A CPU percentage paired with a memory env-file limit and two
//! baselines runs both emitters at once, which is where a source could be read
//! for one metric and quietly reused, or a baseline applied to the wrong side.
//!
//! `PCT_TEST_CPU_NETWORK` is `10_000` and `PCT_TEST_MEM_NETWORK` is `20_000` in
//! `tests/ui/support/pass_env_file.env`, so the CPU limit resolves to `2_500`
//! (`reference * pct / 100`) while the memory limit stays `20_000`. The floor
//! subtracted from each raw measurement is `1_000`.

#[path = "../support/mock_env.rs"]
mod mock_env;

use budget_macros::budget_lt;
use mock_env::{budget_panic, Env};

const MIXED_ENV_FILE: &str = "tests/ui/support/pass_env_file.env";
/// The CPU network limit as the env file holds it.
const CPU_NETWORK_LIMIT: u64 = 10_000;
/// The memory network limit as the env file holds it; used verbatim.
const MEM_NETWORK_LIMIT: u64 = 20_000;
/// The percentage of the CPU network limit the attribute asks for.
const CPU_PCT: u64 = 25;
/// What the percentage resolves to under the macro's integer arithmetic.
const CPU_LIMIT: u64 = CPU_NETWORK_LIMIT * CPU_PCT / 100;

/// Stand-in floor for both metrics, evaluated by the macro per measurement.
fn floor() -> u64 {
    1_000
}

/// Marginal CPU 2_000 of the resolved 2_500 and marginal memory 19_500 of
/// 20_000: both sources resolved, both baselines applied, nothing breached.
#[budget_lt(
    cpu = pct = 25,
    of = env_file = MIXED_ENV_FILE,
    env = "PCT_TEST_CPU_NETWORK",
    mem = env_file = MIXED_ENV_FILE,
    env = "PCT_TEST_MEM_NETWORK",
    cpu_baseline = floor(),
    mem_baseline = floor()
)]
fn mixed_sources_both_within_limit() {
    let env = Env::new(3_000, 20_500);
}

/// The CPU side alone breaches its percentage limit.
#[budget_lt(
    cpu = pct = 25,
    of = env_file = MIXED_ENV_FILE,
    env = "PCT_TEST_CPU_NETWORK",
    mem = env_file = MIXED_ENV_FILE,
    env = "PCT_TEST_MEM_NETWORK",
    cpu_baseline = floor(),
    mem_baseline = floor()
)]
fn mixed_sources_cpu_breaches_percentage() {
    let env = Env::new(4_000, 20_500);
}

/// The memory side alone breaches the limit taken straight from the env file,
/// with the CPU cost comfortably under its percentage limit.
#[budget_lt(
    cpu = pct = 25,
    of = env_file = MIXED_ENV_FILE,
    env = "PCT_TEST_CPU_NETWORK",
    mem = env_file = MIXED_ENV_FILE,
    env = "PCT_TEST_MEM_NETWORK",
    cpu_baseline = floor(),
    mem_baseline = floor()
)]
fn mixed_sources_mem_breaches_env_file_limit() {
    let env = Env::new(2_000, 22_000);
}

/// A percentage breach reports the marginal cost, the *resolved* limit, the
/// raw measurement and the baseline, and the percentage it came from.
///
/// The absence of `exceeded limit 10000` is the point: the env-file value is
/// the reference the percentage is taken of, never the limit itself.
fn assert_cpu_breach() {
    let message = budget_panic(mixed_sources_cpu_breaches_percentage)
        .expect("the CPU percentage assertion should have failed");
    assert!(
        message.contains(&format!(
            "CPU instruction cost {} exceeded limit {CPU_LIMIT}",
            4_000 - floor()
        )),
        "the CPU breach must report the marginal cost against the resolved \
         percentage limit: {message}"
    );
    assert!(
        message.contains("4000 measured - 1000 baseline"),
        "the CPU breach must show the raw measurement and the CPU baseline: {message}"
    );
    assert!(
        message.contains("25% of network limit"),
        "the CPU breach must name the percentage that produced the limit: {message}"
    );
    assert!(
        !message.contains("exceeded limit 10000"),
        "the env-file value is the percentage's reference, not the CPU limit: {message}"
    );
}

/// A memory breach reports the env-file value as the limit, after the memory
/// baseline, and carries no percentage — the two metrics keep their own
/// sources.
fn assert_mem_breach() {
    let message = budget_panic(mixed_sources_mem_breaches_env_file_limit)
        .expect("the memory env-file assertion should have failed");
    assert!(
        message.contains(&format!(
            "Memory bytes cost {} exceeded limit {MEM_NETWORK_LIMIT}",
            22_000 - floor()
        )),
        "the memory breach must report the marginal cost against the env-file limit: {message}"
    );
    assert!(
        message.contains("22000 measured - 1000 baseline"),
        "the memory breach must show the raw measurement and the memory baseline: {message}"
    );
    assert!(
        !message.contains("%"),
        "the memory limit came from the env file, not a percentage: {message}"
    );
    assert!(
        !message.contains("CPU instruction cost"),
        "the memory breach must not report the CPU metric, which is under its limit: {message}"
    );
}

fn main() {
    mixed_sources_both_within_limit();
    assert_cpu_breach();
    assert_mem_breach();
}
