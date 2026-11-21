//! `env_ident` on the six standalone budget attributes.
//!
//! Issue #989: `StandaloneSpec` carried only a limit and a baseline, so a test
//! or method that binds the environment to anything other than `env` could not
//! use `budget_cpu_lt`, `budget_mem_lt`, `budget_write_bytes_lt`,
//! `budget_read_bytes_lt`, `budget_events_lt` or `budget_ledger_entries_lt` —
//! even though `budget_lt` already honoured `env_ident`. Each attribute now
//! takes `, env_ident = <binding>`, and the default stays `env`.
//!
//! Covered here:
//!   * every one of the six attributes reading a non-`env` binding, under the
//!     limit and over it;
//!   * the binding reached through an `impl` block (#988), so a block-level
//!     limit and an explicit binding combine;
//!   * `baseline` plus `env_ident` in both orders;
//!   * that a differently named binding is genuinely read: the six bare
//!     functions also bind an `env` whose cost breaches the limit, so an
//!     expansion that still read the default binding panics on the wrong
//!     figure instead of passing quietly.

#[path = "../support/mock_env.rs"]
mod mock_env;

use budget_macros::{
    budget_cpu_lt, budget_events_lt, budget_ledger_entries_lt, budget_mem_lt, budget_read_bytes_lt,
    budget_write_bytes_lt,
};
// `soroban_sdk` looks unused but is not: the ledger-entry expansion emits the
// fully-qualified `soroban_sdk::testutils::budget::ContractCostType` path,
// resolved through the mock's shim re-export.
use mock_env::{budget_panic, Env, soroban_sdk};

struct CpuContract;
struct LedgerContract;

fn floor() -> u64 {
    500
}

/// The binding is named `e` here, never `env`: an expansion that still read a
/// hardcoded `env` would fail to compile rather than pass quietly.
#[budget_cpu_lt(1_000, env_ident = e)]
impl CpuContract {
    fn under_the_named_binding_limit() {
        let e = Env::new(900, 0);
        let _ = e.cost_estimate().budget().cpu_instruction_cost();
    }

    fn over_the_named_binding_limit() {
        let e = Env::new(1_500, 0);
        let _ = e.cost_estimate().budget().cpu_instruction_cost();
    }
}

#[budget_ledger_entries_lt(6, env_ident = e)]
impl LedgerContract {
    fn under_the_named_binding_limit() {
        let e = Env::new_full(0, 0, 0, 4, 1);
        let _ = e.cost_estimate().budget().cpu_instruction_cost();
    }

    fn over_the_named_binding_limit() {
        let e = Env::new_full(0, 0, 0, 5, 3);
        let _ = e.cost_estimate().budget().cpu_instruction_cost();
    }
}

// ---- the remaining four attributes, as bare functions ----
//
// Each also binds a variable literally named `env` whose cost breaches the
// limit, so an expansion that still read the default binding is a failure here
// rather than a silent pass.

#[budget_mem_lt(1_000, env_ident = store)]
fn mem_reads_the_named_binding() {
    let store = Env::new(0, 900);
    let env = Env::new(0, 9_000);
}

#[budget_mem_lt(1_000, env_ident = store)]
fn mem_over_limit_on_the_named_binding() {
    let store = Env::new(0, 1_500);
    let env = Env::new(0, 9_000);
}

#[budget_write_bytes_lt(1_000, env_ident = ctx)]
fn write_bytes_reads_the_named_binding() {
    let ctx = Env::new(0, 900);
    let env = Env::new(0, 9_000);
}

#[budget_read_bytes_lt(1_000, env_ident = ctx)]
fn read_bytes_reads_the_named_binding() {
    let ctx = Env::new(0, 900);
    let env = Env::new(0, 9_000);
}

#[budget_events_lt(4, env_ident = host)]
fn events_reads_the_named_binding() {
    let host = Env::new_full(0, 0, 3, 0, 0);
    let env = Env::new_full(0, 0, 90, 0, 0);
}

#[budget_events_lt(4, env_ident = host)]
fn events_over_limit_on_the_named_binding() {
    let host = Env::new_full(0, 0, 5, 0, 0);
    let env = Env::new_full(0, 0, 90, 0, 0);
}

// ---- `baseline` combined with `env_ident`, in both orders ----

#[budget_cpu_lt(1_000, baseline = floor(), env_ident = e)]
fn baseline_before_env_ident() {
    // 1_600 measured - 500 baseline = 1_100 marginal, over 1_000.
    let e = Env::new(1_600, 0);
    let _ = e.cost_estimate().budget().cpu_instruction_cost();
}

#[budget_cpu_lt(1_000, env_ident = e, baseline = floor())]
fn env_ident_before_baseline() {
    let e = Env::new(1_400, 0);
    let _ = e.cost_estimate().budget().cpu_instruction_cost();
}

/// Assert a failure message carries the expected measurement text; `case` only
/// labels the assertion output.
fn assert_failure<F, R>(case: &str, expected: &str, f: F)
where
    F: FnOnce() -> R + std::panic::UnwindSafe,
{
    let message = budget_panic(f).unwrap_or_else(|| panic!("{case}: should have failed"));
    assert!(
        message.contains(expected),
        "{case}: unexpected panic message: {message}"
    );
}

fn main() {
    CpuContract::under_the_named_binding_limit();
    LedgerContract::under_the_named_binding_limit();
    mem_reads_the_named_binding();
    write_bytes_reads_the_named_binding();
    read_bytes_reads_the_named_binding();
    events_reads_the_named_binding();

    // One failing method per metric family is enough to pin the wiring, since
    // each carries its own metric phrase.
    assert_failure(
        "cpu via `env_ident`",
        "CPU instruction cost 1500 exceeded limit 1000",
        CpuContract::over_the_named_binding_limit,
    );
    assert_failure(
        "mem via `env_ident`",
        "Memory bytes cost 1500 exceeded limit 1000",
        mem_over_limit_on_the_named_binding,
    );
    assert_failure(
        "events via `env_ident`",
        "Event count 5 exceeded limit 4",
        events_over_limit_on_the_named_binding,
    );
    assert_failure(
        "ledger entries via `env_ident`",
        "Ledger entry count (read: 5, write: 3, total: 8) exceeded limit 6",
        LedgerContract::over_the_named_binding_limit,
    );

    // `baseline` + `env_ident` combine, and the order of the two keys is free.
    assert_failure(
        "baseline before env_ident",
        "CPU instruction cost 1100 exceeded limit 1000 (marginal: 1600 measured - 500 baseline)",
        baseline_before_env_ident,
    );
    env_ident_before_baseline();
}
