//! Env-file form (`env_file = "PATH"` + `env = "VAR"`) of
//! `#[budget_events_lt]`.
//!
//! `pass_env_file.rs` proves that limit form for `budget_cpu_lt`; the event
//! metric builds its assertion through the same `BudgetLimit::EnvFile` arm, and
//! this fixture is where that is observed. Three scenarios:
//!
//! 1. A present key whose value is far above the mock event count, so the
//!    assertion passes on the file's value.
//! 2. A present key whose value the mock event count crosses, so the file's
//!    number is what the failure reports — a limit read from the environment or
//!    defaulted away would not produce it.
//! 3. A key absent from the file, whose panic names both the file and the key.
//!
//! The file is shared with `pass_env_file.rs` rather than duplicated, and no key
//! is added to it: `TINY_LIMIT` and `PCT_TEST_SMALL_NETWORK` already bracket the
//! event counts used here.

#[path = "../support/mock_env.rs"]
mod mock_env;

use budget_macros::budget_events_lt;
use mock_env::{budget_panic, Env};

// trybuild runs the pass fixtures from a generated crate under the target
// directory, so the literal path is missed and the file is found through the
// `../../../../budget-macros/…` candidate the macro emits — the same walk
// `pass_env_file.rs` relies on.
const ENV_FILE: &str = "tests/ui/support/pass_env_file.env";

/// `TINY_LIMIT` is 1_000_000, so three events stay well under the file's value.
#[budget_events_lt(env_file = ENV_FILE, env = "TINY_LIMIT")]
fn event_count_under_the_file_limit() {
    let env = Env::new_full(0, 0, 3, 0, 0);
}

/// `PCT_TEST_SMALL_NETWORK` is 50, so 60 events must fail against it.
#[budget_events_lt(env_file = ENV_FILE, env = "PCT_TEST_SMALL_NETWORK")]
fn event_count_over_the_file_limit() {
    let env = Env::new_full(0, 0, 60, 0, 0);
}

/// A key the checked-in file does not carry.
#[budget_events_lt(env_file = ENV_FILE, env = "MISSING_EVENTS_KEY")]
fn missing_key_reports_the_file_and_the_key() {
    let env = Env::new_full(0, 0, 1, 0, 0);
}

/// Runs `case`, requiring it to panic, and asserts the panic message names every
/// fragment in `expected`.
fn assert_panics_mentioning<F, R>(case: &str, expected: &[&str], f: F)
where
    F: FnOnce() -> R + std::panic::UnwindSafe,
{
    let message = budget_panic(f)
        .unwrap_or_else(|| panic!("{case}: the budget assertion should have panicked"));
    for fragment in expected {
        assert!(
            message.contains(fragment),
            "{case}: panic message must name `{fragment}`: {message}"
        );
    }
}

fn main() {
    event_count_under_the_file_limit();

    assert_panics_mentioning(
        "present key over limit",
        &["Event count 60 exceeded limit 50"],
        event_count_over_the_file_limit,
    );

    assert_panics_mentioning(
        "missing key",
        &["MISSING_EVENTS_KEY", "pass_env_file.env"],
        missing_key_reports_the_file_and_the_key,
    );
}
