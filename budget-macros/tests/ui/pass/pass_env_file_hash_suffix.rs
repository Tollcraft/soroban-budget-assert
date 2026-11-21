//! An inline env-file comment must be separated from its value by a space.
//!
//! Issue #998: the generated reader strips an inline comment with
//! `raw.find(" #")` — the *space* is the separator, not the `#`. A line written
//! `LIMIT=1000#note` therefore keeps the whole tail as the value, and the
//! value is then handed to `parse::<u64>()`, which panics naming the unstripped
//! raw string. `cargo budget-report --derive-limits` writes
//! `KEY=VALUE # derived …` with the space, so the tool is safe; hand-edits and
//! other generators are not.
//!
//! This fixture pins that behaviour rather than changing it: the space form
//! resolves, the no-space forms fail loudly with the raw value in the
//! diagnostic, and quoting does not rescue a missing space because comments are
//! stripped before quotes.
//!
//! The rule to keep in mind when writing a limits file: `KEY=VALUE # note` is
//! a value of `VALUE`; `KEY=VALUE# note` is a value of `VALUE# note`.

#[path = "../support/mock_env.rs"]
mod mock_env;

use budget_macros::budget_cpu_lt;
use mock_env::{budget_panic, Env};

// Resolved against the crate's working directory (`budget-macros/`) at test
// runtime, which is what trybuild uses when it builds *and runs* pass fixtures.
const ENV_FILE: &str = "tests/ui/support/pass_env_file_hash_suffix.env";

/// A mock cost small enough that every resolvable limit here passes: the only
/// way these cases can fail is the value itself.
const COST: u64 = 500;

#[budget_cpu_lt(env_file = ENV_FILE, env = "WITH_SPACE_LIMIT")]
fn space_prefixed_comment_is_stripped() {
    let env = Env::new(COST, 0);
    let _ = env.cost_estimate().budget().cpu_instruction_cost();
}

#[budget_cpu_lt(env_file = ENV_FILE, env = "QUOTED_WITH_SPACE_LIMIT")]
fn quoted_value_with_a_space_comment_resolves_after_the_comment() {
    // Comment stripping happens first, quote stripping second, so this is
    // `"1000000"` -> 1000000.
    let env = Env::new(COST, 0);
    let _ = env.cost_estimate().budget().cpu_instruction_cost();
}

#[budget_cpu_lt(env_file = ENV_FILE, env = "NO_SPACE_LIMIT")]
fn comment_without_a_space_is_part_of_the_value() {
    let env = Env::new(COST, 0);
    let _ = env.cost_estimate().budget().cpu_instruction_cost();
}

#[budget_cpu_lt(env_file = ENV_FILE, env = "BARE_HASH_LIMIT")]
fn trailing_hash_with_nothing_after_it_is_still_not_a_number() {
    let env = Env::new(COST, 0);
    let _ = env.cost_estimate().budget().cpu_instruction_cost();
}

#[budget_cpu_lt(env_file = ENV_FILE, env = "QUOTED_NO_SPACE_LIMIT")]
fn quoting_does_not_rescue_a_missing_space() {
    // `"1000000"#note` starts with a quote but does not end with one, so the
    // quote strip does not apply and the raw value reaches the parse.
    let env = Env::new(COST, 0);
    let _ = env.cost_estimate().budget().cpu_instruction_cost();
}

/// Runs `case` and returns the panic message it produced.
fn panic_message<F, R>(case: &str, f: F) -> String
where
    F: FnOnce() -> R + std::panic::UnwindSafe,
{
    budget_panic(f).unwrap_or_else(|| panic!("{case}: expected a panic naming the raw value"))
}

fn main() {
    space_prefixed_comment_is_stripped();
    quoted_value_with_a_space_comment_resolves_after_the_comment();

    // Each failure must name the macro, the file, the key, and the exact
    // unstripped value — the whole point of the diagnostic.
    let no_space = panic_message("NO_SPACE_LIMIT", comment_without_a_space_is_part_of_the_value);
    assert!(
        no_space.contains(
            "budget_cpu_lt: env_file tests/ui/support/pass_env_file_hash_suffix.env \
             key NO_SPACE_LIMIT=\"1000000#derived-by-cargo-budget-report\" \
             is not a valid u64"
        ),
        "NO_SPACE_LIMIT: unexpected diagnostic: {no_space}"
    );

    let bare_hash = panic_message("BARE_HASH_LIMIT", trailing_hash_with_nothing_after_it_is_still_not_a_number);
    assert!(
        bare_hash.contains("key BARE_HASH_LIMIT=\"1000000#\" is not a valid u64"),
        "BARE_HASH_LIMIT: unexpected diagnostic: {bare_hash}"
    );

    let quoted = panic_message("QUOTED_NO_SPACE_LIMIT", quoting_does_not_rescue_a_missing_space);
    assert!(
        quoted.contains("key QUOTED_NO_SPACE_LIMIT="),
        "QUOTED_NO_SPACE_LIMIT: the key must be named: {quoted}"
    );
    assert!(
        quoted.contains("#note\" is not a valid u64"),
        "the raw value must reach the diagnostic unmodified: {quoted}"
    );

    // The panics are value failures, never limit breaches: a case that read a
    // resolved limit would say "exceeded limit" instead.
    for message in [&no_space, &bare_hash, &quoted] {
        assert!(
            !message.contains("exceeded limit"),
            "the limit must not be reached: {message}"
        );
    }
}
