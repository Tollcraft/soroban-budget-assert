//! Early `return`s: every path that leaves the test runs the assertion, so a
//! budget regression can no longer hide behind an early exit.

#[path = "../support/mock_env.rs"]
mod mock_env;

use budget_macros::budget_cpu_lt;
use mock_env::{budget_panic, Env};

#[derive(Debug, PartialEq)]
struct TestError;

#[budget_cpu_lt(1_000)]
fn early_return_unit(exit_early: bool) {
    let env = Env::new(1_001, 0);
    if exit_early {
        return;
    }
    assert!(env.cost_estimate().budget().cpu_instruction_cost() > 0);
}

#[budget_cpu_lt(1_000)]
fn early_return_value(exit_early: bool) -> Result<&'static str, TestError> {
    let env = Env::new(1_001, 0);
    if exit_early {
        return Ok("early");
    }
    Ok("late")
}

#[budget_cpu_lt(2_000)]
fn early_return_under_limit(exit_early: bool) -> Result<&'static str, TestError> {
    let env = Env::new(1_001, 0);
    if exit_early {
        return Ok("early");
    }
    Ok("late")
}

/// A `return` nested in a loop inside a match arm is still a test exit.
#[budget_cpu_lt(1_000)]
fn early_return_nested(limit: u32) -> u32 {
    let env = Env::new(1_001, 0);
    match limit {
        0 => 0,
        n => {
            for i in 0..n {
                if i == 2 {
                    return i;
                }
            }
            n
        }
    }
}

/// A `return` inside a closure exits the closure, not the test, so it must not
/// pick up the assertion.
#[budget_cpu_lt(1_000)]
fn return_inside_closure() -> u64 {
    let env = Env::new(999, 0);
    let first_big = |values: &[u64]| -> u64 {
        for &value in values {
            if value > 2 {
                return value;
            }
        }
        0
    };
    first_big(&[1, 2, 5])
}

fn main() {
    assert_eq!(early_return_under_limit(true), Ok("early"));
    assert_eq!(early_return_under_limit(false), Ok("late"));
    assert_eq!(return_inside_closure(), 5);

    for (name, message) in [
        ("early_return_unit", budget_panic(|| early_return_unit(true))),
        (
            "early_return_value",
            budget_panic(|| early_return_value(true)),
        ),
        (
            "early_return_nested",
            budget_panic(|| early_return_nested(5)),
        ),
    ] {
        let message = message.unwrap_or_else(|| {
            panic!("{name}: the budget assertion should have failed on the early return")
        });
        assert!(
            message.contains("CPU instruction cost 1001 exceeded limit 1000"),
            "{name}: unexpected panic message: {message}"
        );
    }
}
