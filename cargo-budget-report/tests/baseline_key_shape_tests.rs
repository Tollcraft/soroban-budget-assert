//! Integration tests for issue #1028:
//! Reject malformed baseline keys during `Baseline::parse`, emitting an error that
//! names the invalid header and informs the user of the required `"pkg::function"` format.

#[path = "../src/compare.rs"]
mod compare;

use compare::Baseline;

#[test]
fn test_valid_quoted_package_function_keys_parse() {
    let toml = r#"
["amm-pool-contract::swap"]
cpu_instructions = 1000
read_bytes = 2000
write_bytes = 3000

["another-crate::init"]
cpu_instructions = 500
read_bytes = 600
write_bytes = 700
"#;

    let baseline = Baseline::parse(toml).expect("valid package::function keys must parse");
    assert_eq!(baseline.entries.len(), 2);
    assert!(baseline.entries.contains_key("amm-pool-contract::swap"));
    assert!(baseline.entries.contains_key("another-crate::init"));
}

#[test]
fn test_dot_separated_key_rejected() {
    let toml = r#"
["amm-pool-contract.swap"]
cpu_instructions = 1000
read_bytes = 2000
write_bytes = 3000
"#;

    let err = Baseline::parse(toml)
        .expect_err("dot-separated key should be rejected")
        .to_string();

    assert!(
        err.contains("amm-pool-contract.swap"),
        "expected error to name invalid header, got: {err}"
    );
    assert!(
        err.contains("package::function"),
        "expected error to guide with 'package::function', got: {err}"
    );
}

#[test]
fn test_bare_dot_table_rejected() {
    let toml = r#"
[amm_pool_contract.swap]
cpu_instructions = 1000
read_bytes = 2000
write_bytes = 3000
"#;

    let err = Baseline::parse(toml)
        .expect_err("bare dot table should be rejected")
        .to_string();

    assert!(
        err.contains("amm_pool_contract"),
        "expected error to name invalid header, got: {err}"
    );
    assert!(
        err.contains("package::function"),
        "expected error to guide with 'package::function', got: {err}"
    );
}

#[test]
fn test_missing_separator_rejected() {
    let toml = r#"
["just_a_function"]
cpu_instructions = 1000
read_bytes = 2000
write_bytes = 3000
"#;

    let err = Baseline::parse(toml)
        .expect_err("key without :: separator should be rejected")
        .to_string();

    assert!(
        err.contains("just_a_function"),
        "expected error to name invalid header, got: {err}"
    );
    assert!(
        err.contains("package::function"),
        "expected error to guide with 'package::function', got: {err}"
    );
}

#[test]
fn test_multiple_separators_rejected() {
    let toml = r#"
["crate::module::function"]
cpu_instructions = 1000
read_bytes = 2000
write_bytes = 3000
"#;

    let err = Baseline::parse(toml)
        .expect_err("key with multiple :: separators should be rejected")
        .to_string();

    assert!(
        err.contains("crate::module::function"),
        "expected error to name invalid header, got: {err}"
    );
    assert!(
        err.contains("package::function"),
        "expected error to guide with 'package::function', got: {err}"
    );
}
