//! Env_file form (`env_file = "PATH"` + `env = "VAR"`) of the budget
//! `cpu`/`mem` limit attributes.
//!
//! Pattern mirrors `pass_unit.rs` / `pass_mem.rs`: free functions carrying
//! the macro attribute are exercised from `main`, with `budget_panic` used
//! to capture the exact panic message emitted by the macro-generated
//! assertion body. Three scenarios are covered:
//!
//! 1. A present key (`TINY_LIMIT`) with a large value reads successfully and
//!    the assertion passes under the mock cost.
//! 2. A present key whose value bounds the cost (`PCT_TEST_CPU_NETWORK` = 10000)
//!    panics, proving the file's value is enforced rather than ignored.
//! 3. A missing key (`MISSING_KEY`) returns the diagnostic panic message
//!    naming the file path and key, so a reviewer can see what is wired
//!    wrong without re-running the test by hand.
//!
//! The scenarios are split into `run_passing_cases` / `run_limit_enforced_case`
//! / `run_missing_key_case` and a shared `assert_panics_mentioning` helper so
//! each is an independently reviewable unit rather than one nested block in
//! `main`.

#[path = "../support/mock_env.rs"]
mod mock_env;

use budget_macros::budget_cpu_lt;
use mock_env::{budget_panic, Env};

// Path is resolved at runtime against the crate's working directory, which
// is `budget-macros/` when trybuild runs these pass tests.
const ENV_FILE: &str = "tests/ui/support/pass_env_file.env";

/// Fragments the missing-key panic must name: the absent key and the env file
/// it was looked up in. Keeping them together means a new required fragment is
/// a one-line change.
const MISSING_KEY_DIAGNOSTIC: &[&str] = &["MISSING_KEY", "pass_env_file.env"];

#[budget_cpu_lt(env_file = ENV_FILE, env = "TINY_LIMIT")]
fn env_file_present_and_passes() {
    // 999 < TINY_LIMIT (1,000,000) so the assertion must not panic.
    let env = Env::new(999, 0);
    let _ = env.cost_estimate().budget().cpu_instruction_cost();
}

#[budget_cpu_lt(env_file = ENV_FILE, env = "PCT_TEST_CPU_NETWORK")]
fn env_file_present_key_enforces_limit() {
    // 10_001 > PCT_TEST_CPU_NETWORK (10,000), so the file's value must trigger
    // the assertion. A regression that ignored the value and fell back to "no
    // limit" would only be caught by this case, not by case (1).
    let env = Env::new(10_001, 0);
    let _ = env.cost_estimate().budget().cpu_instruction_cost();
}

#[budget_cpu_lt(env_file = ENV_FILE, env = "MISSING_KEY")]
fn env_file_missing_key_panics_diagnostically() {
    let env = Env::new(0, 0);
    let _ = env.cost_estimate().budget().cpu_instruction_cost();
}

/// Runs `case`, requiring it to panic, and asserts the panic message names
/// every fragment in `expected`. Checking each fragment separately keeps the
/// failure diagnostic specific about what the macro stopped emitting.
fn assert_panics_mentioning<F, R>(case: &str, expected: &[&str], f: F)
where
    F: FnOnce() -> R + std::panic::UnwindSafe,
{
    let message =
        budget_panic(f).unwrap_or_else(|| panic!("{case}: the budget assertion should have panicked"));
    for fragment in expected {
        assert!(
            message.contains(fragment),
            "{case}: panic message must name `{fragment}`: {message}"
        );
    }
}

/// (1) Env file read succeeds when the key is present and within budget.
fn run_passing_cases() {
    env_file_present_and_passes();
}

/// (2) A present key's value is enforced: the limit read from the file bounds
/// the reported cost, and the panic names both numbers.
fn run_limit_enforced_case() {
    assert_panics_mentioning(
        "present key over limit",
        &["CPU instruction cost 10001 exceeded limit 10000"],
        env_file_present_key_enforces_limit,
    );
}

/// (3) Missing key produces an actionable panic: the message names the file
/// path and the missing key, so a contributor who broke the wiring can see
/// both at once.
fn run_missing_key_case() {
    assert_panics_mentioning(
        "missing key",
        MISSING_KEY_DIAGNOSTIC,
        env_file_missing_key_panics_diagnostically,
    );
}

fn main() {
    run_passing_cases();
    run_limit_enforced_case();
    run_missing_key_case();
}
