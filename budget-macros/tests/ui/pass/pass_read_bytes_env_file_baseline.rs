//! `#[budget_read_bytes_lt]` accepts an `env_file` limit *and* a `baseline` at
//! the same time, and the baseline is subtracted before the env-file limit is
//! applied.
//!
//! The two forms each have a fixture of their own, so nothing covered the
//! combination: `pass_read_bytes.rs` and `pass_read_bytes_baseline.rs` pin the
//! baseline against a hardcoded integer limit, while `pass_env_file.rs` pins an
//! env-file limit with no baseline. `StandaloneSpec` (`src/lib.rs`) parses the
//! limit first and then any number of trailing `baseline` / `env_ident` keys,
//! so the combination is legal but unpinned.
//!
//! Four cases, each discriminating on its own:
//!
//! 1. Loose file limit (500), marginal 200: passes, so the combined form
//!    compiles and the assertion is reached.
//! 2. Same key, marginal 900: fails, and the failure text reports the
//!    *marginal* figure, the file's limit, and the `measured - baseline` pair.
//! 3. Tight file limit (100) against the marginal 200 from case 1: fails. The
//!    measurement and baseline are unchanged, so this only fails if the limit
//!    came from the named key in the file rather than being defaulted.
//! 4. Tight file limit (100) with measured 1_050: passes. Raw 1_050 would
//!    breach 100, so a pass here is only possible if the baseline was
//!    subtracted before the env-file limit was compared. This is the ordering
//!    property the two isolated fixtures cannot show.
//!
//! The `.env` file path is resolved at runtime against trybuild's working
//! directory, which is `budget-macros/`, matching the other env-file fixtures.

#[path = "../support/mock_env.rs"]
mod mock_env;

use budget_macros::budget_read_bytes_lt;
use mock_env::{budget_panic, Env};

const ENV_FILE: &str = "tests/ui/support/pass_read_bytes_env_file_baseline.env";

fn floor() -> u64 {
    1_000
}

/// (1) 1_200 measured - 1_000 baseline = 200 marginal, under the file's 500.
#[budget_read_bytes_lt(env_file = ENV_FILE, env = "READ_BYTES_LOOSE_LIMIT", baseline = floor())]
fn marginal_cost_under_file_limit() {
    let env = Env::new(0, 1_200);
}

/// (2) 1_900 - 1_000 = 900 marginal, over the file's 500. The baseline shifts
/// the comparison, it does not disable it.
#[budget_read_bytes_lt(env_file = ENV_FILE, env = "READ_BYTES_LOOSE_LIMIT", baseline = floor())]
fn marginal_cost_over_file_limit() {
    let env = Env::new(0, 1_900);
}

/// (3) The same body as (1) against `READ_BYTES_TIGHT_LIMIT` (100): marginal
/// 200 now breaches it. Only the key changed, so the limit in the message is
/// provably the file's value for that key.
#[budget_read_bytes_lt(env_file = ENV_FILE, env = "READ_BYTES_TIGHT_LIMIT", baseline = floor())]
fn same_marginal_cost_breaches_tighter_file_limit() {
    let env = Env::new(0, 1_200);
}

/// (4) 1_050 - 1_000 = 50 marginal, under the tight 100. Without baseline
/// subtraction the raw 1_050 would breach 100, so this passing is the proof
/// that the env-file limit is applied after the subtraction.
#[budget_read_bytes_lt(env_file = ENV_FILE, env = "READ_BYTES_TIGHT_LIMIT", baseline = floor())]
fn baseline_subtracted_before_file_limit() {
    let env = Env::new(0, 1_050);
}

/// Runs `case`, requiring it to panic, and asserts the message carries every
/// fragment in `expected`. Checking fragments one at a time keeps a failure
/// naming the piece the macro stopped emitting.
fn assert_panics_mentioning<F, R>(case: &str, expected: &[&str], f: F)
where
    F: FnOnce() -> R + std::panic::UnwindSafe,
{
    let message = budget_panic(f)
        .unwrap_or_else(|| panic!("{case}: the budget assertion should have panicked"));
    for fragment in expected {
        assert!(
            message.contains(fragment),
            "{case}: panic message must contain `{fragment}`: {message}"
        );
    }
}

/// (1) and (4) compile, run, and stay inside the limit read from the file.
fn run_passing_cases() {
    marginal_cost_under_file_limit();
    baseline_subtracted_before_file_limit();
}

/// (2) A baseline-reduced breach against an env-file limit reports the marginal
/// number, the file's limit, and the measurement the baseline came off.
fn run_marginal_breach_case() {
    assert_panics_mentioning(
        "marginal cost over file limit",
        &[
            "Read bytes cost (memory proxy) 900 exceeded limit 500",
            "(marginal: 1900 measured - 1000 baseline)",
        ],
        marginal_cost_over_file_limit,
    );
}

/// (3) Same measurement and baseline as (1); only the file key differs, and the
/// tighter value is the limit in the message.
fn run_tighter_file_limit_case() {
    assert_panics_mentioning(
        "same marginal cost, tighter file limit",
        &["Read bytes cost (memory proxy) 200 exceeded limit 100"],
        same_marginal_cost_breaches_tighter_file_limit,
    );
}

fn main() {
    run_passing_cases();
    run_marginal_breach_case();
    run_tighter_file_limit_case();
}
