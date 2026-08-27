//! Percentage-based budget limits: `pct = N, of = env_file = "...", env = "KEY"`.
//!
//! The percentage is resolved against a reference limit read from an env file
//! at test runtime. The env file contains `PCT_TEST_CPU_NETWORK=10000`, so
//! `pct = 25` resolves to 2500. Mock cost of 2499 passes; 2501 fails.
//!
//! The failure message must show the percentage, the resolved absolute limit,
//! and the actual value — all three, since a user reading only the percentage
//! cannot tell how close they were.

#[path = "../support/mock_env.rs"]
mod mock_env;

use budget_macros::budget_cpu_lt;
use mock_env::{budget_panic, Env};

const PCT_ENV_FILE: &str = "tests/ui/support/pass_env_file.env";

// 25% of PCT_TEST_CPU_NETWORK (10000) = 2500. Mock cost 2499 is under.
#[budget_cpu_lt(pct = 25, of = env_file = PCT_ENV_FILE, env = "PCT_TEST_CPU_NETWORK")]
fn pct_passes_when_under() {
    let env = Env::new(2_499, 0);
    let _ = env.cost_estimate().budget().cpu_instruction_cost();
}

// 25% of 10000 = 2500. Mock cost 2501 is over: the assertion must fire.
#[budget_cpu_lt(pct = 25, of = env_file = PCT_ENV_FILE, env = "PCT_TEST_CPU_NETWORK")]
fn pct_fails_when_over() {
    let env = Env::new(2_501, 0);
    let _ = env.cost_estimate().budget().cpu_instruction_cost();
}

// 50% of 10000 = 5000. Mock cost 4999 passes.
#[budget_cpu_lt(pct = 50, of = env_file = PCT_ENV_FILE, env = "PCT_TEST_CPU_NETWORK")]
fn pct_50_passes() {
    let env = Env::new(4_999, 0);
    let _ = env.cost_estimate().budget().cpu_instruction_cost();
}

// 100% of 10000 = 10000. Mock cost 9999 passes.
#[budget_cpu_lt(pct = 100, of = env_file = PCT_ENV_FILE, env = "PCT_TEST_CPU_NETWORK")]
fn pct_100_passes() {
    let env = Env::new(9_999, 0);
    let _ = env.cost_estimate().budget().cpu_instruction_cost();
}

fn main() {
    pct_passes_when_under();
    pct_50_passes();
    pct_100_passes();

    // The failure message must contain all three pieces of information:
    // the percentage (25%), the resolved limit (2500), and the actual value (2501).
    let message = budget_panic(pct_fails_when_over)
        .expect("the percentage budget assertion should have failed");
    assert!(
        message.contains("2501"),
        "failure message must contain the actual value: {message}"
    );
    assert!(
        message.contains("2500"),
        "failure message must contain the resolved limit: {message}"
    );
    assert!(
        message.contains("25%"),
        "failure message must contain the percentage: {message}"
    );
}
