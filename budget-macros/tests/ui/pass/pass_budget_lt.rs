//! Every attribute form `#[budget_lt(...)]` actually accepts.
//!
//! `docs/src/reference.md` used to document five `budget_lt` forms — a
//! positional literal, `env =`, `env_file =` + `env =`, `config =` and
//! `baseline =` — none of which compile, because `budget_lt` parses its
//! arguments as a `BudgetSpec`, whose only keys are `cpu`, `mem`,
//! `env_ident`, `cpu_baseline` and `mem_baseline`, and whose limits must be
//! *nested* under a metric (`cpu = env = "VAR"`). Issue #808 corrected the
//! page; this fixture is the evidence that the forms it now shows compile and
//! enforce the limit each names.
//!
//! Mirrors `pass_unit.rs` / `pass_env_file.rs`: free functions carrying the
//! attribute, driven from `main`, with `budget_panic` capturing the exact
//! diagnostic the generated assertion emits.

#[path = "../support/mock_env.rs"]
mod mock_env;

use budget_macros::budget_lt;
use mock_env::{budget_panic, Env};

// Resolved at runtime against the crate working directory, which is
// `budget-macros/` when trybuild runs this fixture.
const ENV_FILE: &str = "tests/ui/support/pass_env_file.env";
const CONFIG_PATH: &str = "budget.json";

/// `TINY_LIMIT` in the fixture env file.
const TINY_LIMIT: u64 = 1_000_000;

/// 25 % of `PCT_TEST_CPU_NETWORK` (10 000).
const PCT_RESOLVED_LIMIT: u64 = 2_500;

/// A baseline may be any expression, including a call.
fn instantiation_floor() -> u64 {
    1_000
}

// -- literal limits ----------------------------------------------------------

#[budget_lt(cpu = 1_000_000, mem = 1_000_000)]
fn both_metrics_from_literals() {
    let env = Env::new(10, 20);
}

#[budget_lt(cpu = 500)]
fn cpu_only_limit_leaves_memory_unchecked() {
    // Memory is far over any plausible ceiling: at least one of `cpu`/`mem` is
    // required, but the metric that is absent is simply not asserted.
    let env = Env::new(499, 9_999_999);
}

// -- nested sources ----------------------------------------------------------

#[budget_lt(cpu = env = "UI_TEST_MAX_CPU")]
fn cpu_from_process_env() {
    // `UI_TEST_MAX_CPU` is set to 1000 by `tests/ui.rs`.
    let env = Env::new(999, 0);
}

#[budget_lt(cpu = env_file = ENV_FILE, env = "TINY_LIMIT", mem = 1_000)]
fn cpu_from_env_file_mem_from_literal() {
    let env = Env::new(TINY_LIMIT - 1, 999);
}

#[budget_lt(mem = config = "mem_limit")]
fn mem_from_config() {
    let env = Env::new(0, 29);
}

#[budget_lt(cpu = pct = 25, of = env_file = ENV_FILE, env = "PCT_TEST_CPU_NETWORK", mem = 500)]
fn cpu_as_percentage_of_a_network_limit() {
    let env = Env::new(PCT_RESOLVED_LIMIT - 1, 499);
}

// -- per-metric baselines ----------------------------------------------------

#[budget_lt(cpu = 1_000_000, cpu_baseline = instantiation_floor(), mem = 500, mem_baseline = 100)]
fn marginal_limits_subtract_their_own_baseline() {
    // CPU marginal is 1000 - 1000 = 0, memory marginal saturates at 50 - 100 = 0.
    let env = Env::new(1_000, 50);
}

// -- env_ident ---------------------------------------------------------------

#[budget_lt(env_ident = pool_env, cpu = 500)]
fn cost_is_read_from_a_renamed_env_binding() {
    let pool_env = Env::new(100, 0);
}

// -- failure paths -----------------------------------------------------------

#[budget_lt(cpu = 1_000_000, mem = 1_000_000)]
fn over_both_limits() {
    let env = Env::new(1_000_000, 2_000_000);
}

#[budget_lt(cpu = env_file = ENV_FILE, env = "PCT_TEST_MEM_NETWORK", mem = 500)]
fn mem_over_an_env_file_limit() {
    // The `cpu` limit is the 20 000 network memory figure, so the CPU check
    // passes and the memory check (limit 500) is the one that reports.
    let env = Env::new(10, 600);
}

#[budget_lt(cpu = pct = 25, of = env_file = ENV_FILE, env = "PCT_TEST_CPU_NETWORK")]
fn pct_limit_over_when_expanded() {
    let env = Env::new(2_501, 0);
}

fn assert_panics_with<F: FnOnce() -> R + std::panic::UnwindSafe, R>(f: F, expected: &str) {
    let message = budget_panic(f).unwrap_or_else(|| panic!("expected a panic containing {expected}"));
    assert!(
        message.starts_with(expected),
        "diagnostic should start with {expected:?}, got: {message}"
    );
}

fn run_within_limits() {
    both_metrics_from_literals();
    cpu_only_limit_leaves_memory_unchecked();
    cpu_from_process_env();
    cpu_from_env_file_mem_from_literal();
    cpu_as_percentage_of_a_network_limit();
    marginal_limits_subtract_their_own_baseline();
    cost_is_read_from_a_renamed_env_binding();
}

fn run_config_case() {
    let previous_config = std::fs::read(CONFIG_PATH).ok();
    std::fs::write(CONFIG_PATH, r#"{"mem_limit": 30}"#).expect("write fixture budget.json");
    mem_from_config();
    match previous_config {
        Some(previous) => std::fs::write(CONFIG_PATH, previous).expect("restore budget.json"),
        None => std::fs::remove_file(CONFIG_PATH).expect("remove fixture budget.json"),
    }
}

fn run_over_limit_cases() {
    // The CPU assertion is generated first, so a body over both ceilings
    // reports only the CPU breach.
    assert_panics_with(
        over_both_limits,
        "CPU instruction cost 1000000 exceeded limit 1000000",
    );
    assert_panics_with(
        mem_over_an_env_file_limit,
        "Memory bytes cost 600 exceeded limit 500",
    );
    assert_panics_with(
        pct_limit_over_when_expanded,
        "CPU instruction cost 2501 exceeded limit 2500 (25% of network limit)",
    );
}

fn main() {
    run_within_limits();
    run_config_case();
    run_over_limit_cases();
}
