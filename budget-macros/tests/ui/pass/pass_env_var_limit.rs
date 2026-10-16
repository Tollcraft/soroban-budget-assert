//! The `env = "VAR"` form is resolved at macro-expansion time and re-read at
//! test runtime.
//!
//! - An unset variable fails compilation naming the variable; it never
//!   becomes an implicit "no limit".
//! - A set-but-unparsable value fails compilation at expansion time, and
//!   panics at test runtime when the value changes after expansion, with a
//!   diagnostic naming the metric, the variable, and the offending value.
//! - Surrounding whitespace is trimmed before parsing, matching the
//!   `env_file` arm; `" 1000 "` parses as `1000` in both arms.

#[path = "../support/mock_env.rs"]
mod mock_env;

use budget_macros::budget_cpu_lt;
use mock_env::{budget_panic, Env};

#[budget_cpu_lt(env = "UI_TEST_MAX_CPU")]
fn within_limit() {
    let env = Env::new(999, 0);
}

#[budget_cpu_lt(env = "UI_TEST_MAX_CPU_EXCEEDED")]
fn over_limit() {
    let env = Env::new(1_001, 0);
}

#[budget_cpu_lt(env = "UI_TEST_MAX_CPU_UNPARSABLE")]
fn unparsable_limit() {
    let env = Env::new(0, 0);
}

#[budget_cpu_lt(env = "UI_TEST_MAX_CPU_WHITESPACE")]
fn whitespace_limit() {
    let env = Env::new(999, 0);
}

fn main() {
    within_limit();
    let message = budget_panic(over_limit).expect("the budget assertion should have failed");
    assert!(message.contains("CPU instruction cost 1001 exceeded limit 1000"));

    // A set-but-unparsable value aborts the test: the panic names the metric
    // label, the variable, and the offending value.
    std::env::set_var("UI_TEST_MAX_CPU_UNPARSABLE", "1000 Mbps");
    let message =
        budget_panic(unparsable_limit).expect("an unparsable env var should panic");
    assert!(
        message.contains("budget_cpu_lt"),
        "panic must name the metric label: {message}"
    );
    assert!(
        message.contains("UI_TEST_MAX_CPU_UNPARSABLE"),
        "panic must name the variable: {message}"
    );
    assert!(
        message.contains("1000 Mbps"),
        "panic must name the offending value: {message}"
    );
    assert!(
        message.contains("is not a valid u64"),
        "panic must state the parse failure: {message}"
    );

    // Whitespace is trimmed before parsing, matching the `env_file` arm, so a
    // padded value still enforces its numeric limit.
    std::env::set_var("UI_TEST_MAX_CPU_WHITESPACE", " 1000 ");
    whitespace_limit();
}
