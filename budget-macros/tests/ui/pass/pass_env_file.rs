//! Env_file form (`env_file = "PATH"` + `env = "VAR"`) of the budget
//! `cpu`/`mem` limit attributes.
//!
//! Pattern mirrors `pass_unit.rs` / `pass_mem.rs`: free functions carrying
//! the macro attribute are exercised from `main`, with `budget_panic` used
//! to capture the exact panic message emitted by the macro-generated
//! assertion body. Two scenarios are covered:
//!
//! 1. A present key (`TINY_LIMIT`) with a large value reads successfully and
//!    the assertion passes under the mock cost.
//! 2. A missing key (`MISSING_KEY`) returns the diagnostic panic message
//!    naming the file path and key, so a reviewer can see what is wired
//!    wrong without re-running the test by hand.

#[path = "../support/mock_env.rs"]
mod mock_env;

use budget_macros::budget_cpu_lt;
use mock_env::{budget_panic, Env};

// Path is resolved at runtime against the crate's working directory, which
// is `budget-macros/` when trybuild runs these pass tests.
const ENV_FILE: &str = "tests/ui/support/pass_env_file.env";

#[budget_cpu_lt(env_file = ENV_FILE, env = "TINY_LIMIT")]
fn env_file_present_and_passes() {
    // 999 < TINY_LIMIT (1,000,000) so the assertion must not panic.
    let env = Env::new(999, 0);
    let _ = env.cost_estimate().budget().cpu_instruction_cost();
}

#[budget_cpu_lt(env_file = ENV_FILE, env = "MISSING_KEY")]
fn env_file_missing_key_panics_diagnostically() {
    let env = Env::new(0, 0);
    let _ = env.cost_estimate().budget().cpu_instruction_cost();
}

fn main() {
    // (1) Env file read succeeds when the key is present and within budget.
    env_file_present_and_passes();

    // (2) Missing key produces an actionable panic: the message names the
    // file path and the missing key, so a contributor who broke the wiring
    // can see both at once.
    let message = budget_panic(env_file_missing_key_panics_diagnostically)
        .expect("the macro should panic when the requested key is missing");
    assert!(
        message.contains("MISSING_KEY"),
        "panic message must name the missing key: {message}"
    );
    assert!(
        message.contains("pass_env_file.env"),
        "panic message must name the env file path: {message}"
    );
}
