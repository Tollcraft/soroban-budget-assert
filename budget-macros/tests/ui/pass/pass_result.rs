//! `Result`-returning bodies: the trailing expression stays the function's
//! value and the assertion runs after it is evaluated.

#[path = "../support/mock_env.rs"]
mod mock_env;

use budget_macros::budget_cpu_lt;
use mock_env::{budget_panic, Env};

#[derive(Debug, PartialEq)]
struct TestError(&'static str);

fn doubled(n: u64) -> Result<u64, TestError> {
    Ok(n * 2)
}

fn failing() -> Result<u64, TestError> {
    Err(TestError("boom"))
}

#[budget_cpu_lt(1_000)]
fn tail_ok() -> Result<(), TestError> {
    let env = Env::new(999, 0);
    Ok(())
}

#[budget_cpu_lt(1_000)]
fn tail_ok_over_limit() -> Result<(), TestError> {
    let env = Env::new(1_001, 0);
    Ok(())
}

/// `?` keeps working: it borrows nothing from the assertion and the value it
/// produces is still available to the trailing expression.
#[budget_cpu_lt(1_000)]
fn question_mark() -> Result<u64, TestError> {
    let env = Env::new(999, 0);
    let value = doubled(21)?;
    Ok(value)
}

/// A propagating `?` leaves before the assertion, but the test fails on the
/// returned error anyway.
#[budget_cpu_lt(1)]
fn question_mark_propagates() -> Result<u64, TestError> {
    let env = Env::new(u64::MAX, 0);
    let value = failing()?;
    Ok(value)
}

/// The trailing expression can be block-like and can own locals the assertion
/// does not touch.
#[budget_cpu_lt(1_000)]
fn tail_match() -> Result<String, TestError> {
    let env = Env::new(999, 0);
    let owned = String::from("pool");
    match owned.len() {
        4 => Ok(owned),
        _ => Err(TestError("unexpected length")),
    }
}

fn main() {
    assert_eq!(tail_ok(), Ok(()));
    assert_eq!(question_mark(), Ok(42));
    assert_eq!(question_mark_propagates(), Err(TestError("boom")));
    assert_eq!(tail_match(), Ok(String::from("pool")));

    let message =
        budget_panic(tail_ok_over_limit).expect("the budget assertion should have failed");
    assert!(
        message.contains("CPU instruction cost 1001 exceeded limit 1000"),
        "unexpected panic message: {message}"
    );
}
