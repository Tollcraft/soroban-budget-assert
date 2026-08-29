//! `#[budget_read_bytes_lt]` supports `baseline = <expr>` exactly as its
//! siblings do: the baseline is subtracted from the measured read-bytes proxy
//! (`memory_bytes_cost()`) before the comparison, so the *marginal* cost is
//! what gets asserted. `pass_baseline.rs` covers cpu / mem / write bytes; this
//! is the read-bytes case, kept in its own file so it does not touch fixtures
//! other issues also edit.

#[path = "../support/mock_env.rs"]
mod mock_env;

use budget_macros::budget_read_bytes_lt;
use mock_env::{budget_panic, Env};

fn floor() -> u64 {
    1_000
}

// 1_200 measured - 1_000 baseline = 200 marginal, under the 500 limit.
#[budget_read_bytes_lt(500, baseline = floor())]
fn within_limit_after_baseline() {
    let env = Env::new(0, 1_200);
}

// 1_900 - 1_000 = 900 marginal, over the 500 limit: the baseline shifts the
// comparison, it does not disable it.
#[budget_read_bytes_lt(500, baseline = floor())]
fn over_limit_after_baseline() {
    let env = Env::new(0, 1_900);
}

// A measurement below the baseline saturates to 0 rather than wrapping.
#[budget_read_bytes_lt(500, baseline = floor())]
fn below_baseline_saturates() {
    let env = Env::new(0, 10);
}

fn main() {
    within_limit_after_baseline();
    below_baseline_saturates();

    let message = budget_panic(over_limit_after_baseline)
        .expect("the budget assertion should have failed");
    assert!(
        message.contains("Read bytes cost (memory proxy) 900 exceeded limit 500"),
        "the marginal figure should be the asserted one: {message}"
    );
    assert!(
        message.contains("1900 measured - 1000 baseline"),
        "the message should show the raw measurement and the baseline: {message}"
    );
}
