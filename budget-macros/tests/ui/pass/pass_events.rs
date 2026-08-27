//! `#[budget_events_lt]` support and body-shape coverage against the mock `Env`.
//!
//! The event count is read from `env.events().all().events().len()` — a real
//! count, exercised here against the mock.

#[path = "../support/mock_env.rs"]
mod mock_env;

use budget_macros::budget_events_lt;
use mock_env::{budget_panic, Env};

#[derive(Debug, PartialEq)]
struct TestError;

#[budget_events_lt(10)]
fn unit_body() {
    let env = Env::new_full(0, 0, 3, 0, 0);
    let _ = env.events().all().events().len();
}

#[budget_events_lt(10)]
fn result_body() -> Result<u64, TestError> {
    let env = Env::new_full(0, 0, 3, 0, 0);
    Ok(env.events().all().events().len() as u64)
}

#[budget_events_lt(2)]
fn over_limit_panics() {
    let env = Env::new_full(0, 0, 5, 0, 0);
    let _ = env.events().all().events().len();
}

fn main() {
    unit_body();
    assert_eq!(result_body(), Ok(3));

    let message =
        budget_panic(over_limit_panics).expect("the event assertion should have failed");
    assert!(
        message.contains("Event count 5 exceeded limit 2"),
        "unexpected panic message: {message}"
    );
}
