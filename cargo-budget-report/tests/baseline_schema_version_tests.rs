#[path = "../src/compare.rs"]
mod compare;

use compare::Baseline;

#[test]
fn test_to_toml_includes_schema_version() {
    let baseline = Baseline::default();
    let toml_str = baseline.to_toml().expect("to_toml should succeed");
    assert!(
        toml_str.contains("schema_version = 1"),
        "to_toml output must include schema_version = 1, got:\n{toml_str}"
    );
}

#[test]
fn test_parse_versioned_baseline() {
    let toml_str = r#"
schema_version = 1

["pkg::func"]
cpu_instructions = 100
read_bytes = 200
write_bytes = 300
"#;
    let parsed = Baseline::parse(toml_str).expect("versioned baseline should parse");
    assert_eq!(parsed.entries.len(), 1);
    let entry = parsed.entries.get("pkg::func").expect("entry exists");
    assert_eq!(entry.cpu_instructions, 100);
    assert_eq!(entry.read_bytes, 200);
    assert_eq!(entry.write_bytes, 300);
}

#[test]
fn test_parse_legacy_unversioned_baseline() {
    let toml_str = r#"
["pkg::func"]
cpu_instructions = 100
read_bytes = 200
write_bytes = 300
"#;
    let parsed =
        Baseline::parse(toml_str).expect("legacy baseline without schema_version should parse");
    assert_eq!(parsed.entries.len(), 1);
    let entry = parsed.entries.get("pkg::func").expect("entry exists");
    assert_eq!(entry.cpu_instructions, 100);
    assert_eq!(entry.read_bytes, 200);
    assert_eq!(entry.write_bytes, 300);
}

#[test]
fn test_reject_unknown_metric_key() {
    let toml_str = r#"
schema_version = 1

["pkg::func"]
cpu_instructions = 100
read_bytes = 200
write_bytes = 300
unknown_metric = 400
"#;
    let err = Baseline::parse(toml_str).expect_err("unknown metric key must be rejected");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("pkg::func"),
        "error message must name the entry: {msg}"
    );
    assert!(
        msg.contains("unknown_metric"),
        "error message must name the unknown metric key: {msg}"
    );
}

#[test]
fn test_reject_unsupported_schema_version() {
    let toml_str = r#"
schema_version = 2

["pkg::func"]
cpu_instructions = 100
read_bytes = 200
write_bytes = 300
"#;
    let err = Baseline::parse(toml_str).expect_err("unsupported schema_version must be rejected");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("unsupported schema_version 2"),
        "error message must describe unsupported version: {msg}"
    );
}
