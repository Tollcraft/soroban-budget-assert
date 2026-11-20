//! Integration tests for issue #1029:
//! In `cargo-budget-report/src/config.rs`, improve error handling so type deserialization
//! failure errors out or returns an error instead of discarding valid fields.

#[path = "../src/config.rs"]
mod config;

use config::{parse_config, parse_config_fallible, Config};

#[test]
fn test_malformed_type_does_not_discard_valid_fields() {
    let toml = r#"
network = "standalone"
source = "deployer_account"
timeout_secs = "not_a_number"
"#;

    let parsed = parse_config(toml);
    // Valid fields must be preserved
    assert_eq!(parsed.network, "standalone");
    assert_eq!(parsed.source, "deployer_account");
    // Malformed field falls back to default
    assert_eq!(parsed.timeout_secs, Config::default().timeout_secs);
    assert_eq!(parsed.retry_attempts, Config::default().retry_attempts);
}

#[test]
fn test_all_valid_fields_parsed() {
    let toml = r#"
network = "custom-net"
source = "alice"
timeout_secs = 120
retry_attempts = 10
"#;

    let parsed = parse_config(toml);
    assert_eq!(parsed.network, "custom-net");
    assert_eq!(parsed.source, "alice");
    assert_eq!(parsed.timeout_secs, 120);
    assert_eq!(parsed.retry_attempts, 10);
}

#[test]
fn test_fallible_parser_reports_type_error_on_mismatch() {
    let toml = r#"
network = "standalone"
source = "deployer_account"
timeout_secs = "one_hundred"
"#;

    let err = parse_config_fallible(toml).expect_err("should return FieldType error");
    let msg = err.to_string();
    assert!(
        msg.contains("timeout_secs"),
        "expected error to name field timeout_secs, got: {msg}"
    );
}

#[test]
fn test_fallible_parser_reports_syntax_error_on_bad_toml() {
    let toml = "broken = toml = =";
    let err = parse_config_fallible(toml).expect_err("should return Toml error");
    assert!(matches!(err, config::ConfigError::Toml(_)));
}
