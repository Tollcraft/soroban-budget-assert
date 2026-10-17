//! Config-sourced CPU limits parse only non-negative JSON integer values.

#[path = "../support/mock_env.rs"]
mod mock_env;

use budget_macros::budget_cpu_lt;
use mock_env::{budget_panic, Env};

const CONFIG_PATH: &str = "budget.json";

#[budget_cpu_lt(config = "cpu_limit")]
fn configured_limit() {
    let env = Env::new(9, 0);
    let _ = env.cost_estimate().budget().cpu_instruction_cost();
}

#[budget_cpu_lt(config = "limit")]
fn nested_duplicate_key_uses_first_occurrence() {
    let env = Env::new(5, 0);
    let _ = env.cost_estimate().budget().cpu_instruction_cost();
}

#[budget_cpu_lt(config = "cpu")]
fn key_prefix_does_not_match_a_longer_key() {
    let env = Env::new(5, 0);
    let _ = env.cost_estimate().budget().cpu_instruction_cost();
}

fn write_config(contents: &str) {
    std::fs::write(CONFIG_PATH, contents).expect("write test budget.json");
}

fn assert_invalid_value_reports_raw_text(contents: &str, raw: &str) {
    write_config(contents);
    let message = budget_panic(configured_limit)
        .unwrap_or_else(|| panic!("invalid config value {raw:?} should panic"));
    for expected in ["budget_cpu_lt", "cpu_limit", raw] {
        assert!(
            message.contains(expected),
            "diagnostic should contain {expected:?}: {message}"
        );
    }
}

fn run() {
    let previous_config = std::fs::read(CONFIG_PATH).ok();

    write_config(r#"{"cpu_limit": 10   }"#);
    configured_limit();

    assert_invalid_value_reports_raw_text(r#"{"cpu_limit": 1.5}"#, "1.5");
    assert_invalid_value_reports_raw_text(r#"{"cpu_limit": 1e6}"#, "1e6");
    assert_invalid_value_reports_raw_text(r#"{"cpu_limit": -1}"#, "-1");
    assert_invalid_value_reports_raw_text(r#"{"cpu_limit": "1,2"}"#, "1,2");

    write_config(r#"{"another_key": 10}"#);
    let missing = budget_panic(configured_limit).expect("missing key should panic");
    assert!(missing.contains("budget_cpu_lt"), "{missing}");
    assert!(missing.contains("cpu_limit"), "{missing}");

    write_config(r#"{"outer": {"limit": 7}, "limit": 2}"#);
    nested_duplicate_key_uses_first_occurrence();

    write_config(r#"{"cpu": 7, "cpu_limit": 2}"#);
    key_prefix_does_not_match_a_longer_key();

    match previous_config {
        Some(contents) => std::fs::write(CONFIG_PATH, contents).expect("restore budget.json"),
        None => std::fs::remove_file(CONFIG_PATH).expect("remove test budget.json"),
    }
}

fn main() {
    run();
}
