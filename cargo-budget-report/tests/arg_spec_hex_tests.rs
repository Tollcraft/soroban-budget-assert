//! Integration tests for issue #1016: the `bytes` / `bytesn` typed-argument
//! handler rejects odd-length and non-hex values, and the error names the
//! offending value and the argument type.
//!
//! The `0x` prefix is stripped before validation and rendering, so the error
//! text quotes the value exactly as the user wrote it.

#[path = "../src/arg_spec.rs"]
mod arg_spec;

use arg_spec::{render_args, ArgSpec, TypedArg};

fn typed(ty: &str, value: &str) -> ArgSpec {
    ArgSpec::Typed(TypedArg {
        name: "data".to_string(),
        ty: ty.to_string(),
        value: Some(toml::Value::String(value.to_string())),
        generate: false,
    })
}

fn render_error(ty: &str, value: &str) -> String {
    let err = render_args(&[typed(ty, value)], "call").expect_err(&format!(
        "expected `{value}` to be rejected for type `{ty}`"
    ));
    format!("{err:#}")
}

fn assert_rejected_naming_value(ty: &str, value: &str) {
    let msg = render_error(ty, value);
    assert!(
        msg.contains("even-length hex string"),
        "expected hex validation error for `{value}`, got: {msg}"
    );
    assert!(
        msg.contains(ty),
        "expected error to name type `{ty}`, got: {msg}"
    );
    assert!(
        msg.contains(&format!("{value:?}")),
        "expected error to quote offending value `{value}`, got: {msg}"
    );
}

#[test]
fn odd_length_hex_is_rejected_for_bytes() {
    for value in ["0x1", "0xabc", "0x123"] {
        assert_rejected_naming_value("bytes", value);
    }
}

#[test]
fn odd_length_hex_is_rejected_for_bytesn() {
    for value in ["0x1", "0xabc"] {
        assert_rejected_naming_value("bytesn", value);
    }
}

#[test]
fn odd_length_without_prefix_is_rejected() {
    assert_rejected_naming_value("bytes", "abc");
}

#[test]
fn non_hex_characters_are_rejected() {
    for value in ["0xzz", "0xgh", "zz"] {
        assert_rejected_naming_value("bytes", value);
    }
}

#[test]
fn empty_hex_after_prefix_is_rejected() {
    assert_rejected_naming_value("bytes", "0x");
    assert_rejected_naming_value("bytes", "");
}

#[test]
fn even_length_hex_strips_the_prefix_and_renders() {
    let rendered = render_args(&[typed("bytes", "0x1234")], "call")
        .expect("a well-formed even-length hex value should render");
    assert_eq!(rendered, vec!["--data".to_string(), "1234".to_string()]);

    let rendered = render_args(&[typed("bytesn", "abcd")], "call")
        .expect("a prefix-free even-length hex value should render");
    assert_eq!(rendered, vec!["--data".to_string(), "abcd".to_string()]);
}
