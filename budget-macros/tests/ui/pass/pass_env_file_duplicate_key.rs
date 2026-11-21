//! The env-file lookup takes the **first** line whose key matches.
//!
//! Issue #997: `parse_env_file_value` (emitted inside every expansion by
//! `generate_prelude`) walks `content.lines()` and returns from the first line
//! whose left-hand side equals the key, so a duplicated key is not merged, not
//! last-wins, and not an error — the later line is simply never read.
//! `cargo budget-report --derive-limits` appends and rewrites lines, which is
//! how a real `tier-a-limits.env` ends up holding two values for one key.
//! Without this pin, a refactor could flip the iteration order, or adopt
//! last-match quietly, and every derived limit in the suite would change
//! meaning without a failing test.
//!
//! `support/pass_env_file_duplicate_key.env` defines four keys twice each, in
//! both orders, so the two directions are pinned by *behaviour* rather than by
//! a comment:
//!
//!   * `FIRST_WINS_LIMIT` is 1,000,000 then 100 — the case using it must
//!     **pass** at a mock cost of 5,000;
//!   * `SECOND_KEY_LIMIT` is 100 then 1,000,000 — the same cost must
//!     **fail** against the first value;
//!   * `COMMENTED_OUT_LIMIT` is live once and commented once, so the second
//!     spelling is skipped and never a duplicate;
//!   * `SPACED_LIMIT` is written `KEY = 100` on its second line — both sides
//!     of `=` are trimmed, so spacing does not hide the duplicate.
//!
//! Together these mean the effective limit is always the earlier line, and
//! neither the commented-out nor the spaced spelling changes that.

#[path = "../support/mock_env.rs"]
mod mock_env;

use budget_macros::budget_cpu_lt;
use mock_env::{budget_panic, Env};

// Resolved against the crate's working directory (`budget-macros/`) at test
// runtime, which is what trybuild uses when it builds *and runs* pass fixtures.
const ENV_FILE: &str = "tests/ui/support/pass_env_file_duplicate_key.env";

/// A cost that sits between the two spellings of every duplicated key below,
/// so "first wins" and "last wins" produce opposite outcomes.
const COST_BETWEEN_THE_TWO: u64 = 5_000;

#[budget_cpu_lt(env_file = ENV_FILE, env = "FIRST_WINS_LIMIT")]
fn first_definition_is_the_effective_one() {
    // 1_000_000 (line 1) vs 100 (line 2): passes only if line 1 wins.
    let env = Env::new(COST_BETWEEN_THE_TWO, 0);
    let _ = env.cost_estimate().budget().cpu_instruction_cost();
}

#[budget_cpu_lt(env_file = ENV_FILE, env = "SECOND_KEY_LIMIT")]
fn later_definition_is_ignored() {
    // 100 (line 1) vs 1_000_000 (line 2): fails only if line 1 wins.
    let env = Env::new(COST_BETWEEN_THE_TWO, 0);
    let _ = env.cost_estimate().budget().cpu_instruction_cost();
}

#[budget_cpu_lt(env_file = ENV_FILE, env = "COMMENTED_OUT_LIMIT")]
fn commented_duplicate_is_not_a_duplicate() {
    // The `# COMMENTED_OUT_LIMIT=100` line is skipped as a comment, so the
    // limit is the single live 1_000_000.
    let env = Env::new(COST_BETWEEN_THE_TWO, 0);
    let _ = env.cost_estimate().budget().cpu_instruction_cost();
}

#[budget_cpu_lt(env_file = ENV_FILE, env = "SPACED_LIMIT")]
fn spacing_around_the_equals_sign_still_duplicates_the_key() {
    // `SPACED_LIMIT = 100` is the same key; the first (1_000_000) still wins.
    let env = Env::new(COST_BETWEEN_THE_TWO, 0);
    let _ = env.cost_estimate().budget().cpu_instruction_cost();
}

/// Runs `case` expecting no panic.
fn assert_passes<F, R>(case: &str, f: F)
where
    F: FnOnce() -> R + std::panic::UnwindSafe,
{
    if let Some(message) = budget_panic(f) {
        panic!("{case}: expected the first definition to apply, got: {message}");
    }
}

fn main() {
    assert_passes("FIRST_WINS_LIMIT", first_definition_is_the_effective_one);
    assert_passes(
        "COMMENTED_OUT_LIMIT",
        commented_duplicate_is_not_a_duplicate,
    );
    assert_passes("SPACED_LIMIT", spacing_around_the_equals_sign_still_duplicates_the_key);

    let message = budget_panic(later_definition_is_ignored)
        .expect("the second `SECOND_KEY_LIMIT` line must not become the limit");
    assert!(
        message.contains("CPU instruction cost 5000 exceeded limit 100"),
        "the earlier 100 must be the effective limit: {message}"
    );
    assert!(
        !message.contains("1000000"),
        "the later duplicate line must never be read: {message}"
    );
}
