#[path = "../src/arg_spec.rs"]
mod arg_spec;

use arg_spec::{render_args, ArgSpec, TypedArg};
use toml::Value;

fn integer_arg(name: &str, ty: &str, value: i64) -> ArgSpec {
    ArgSpec::Typed(TypedArg {
        name: name.to_string(),
        ty: ty.to_string(),
        value: Some(Value::Integer(value)),
        generate: false,
    })
}

fn render(name: &str, ty: &str, value: i64) -> anyhow::Result<Vec<String>> {
    render_args(&[integer_arg(name, ty, value)], "deposit")
}

#[test]
fn out_of_range_unsigned_integers_are_rejected_before_invocation() {
    for (ty, value) in [
        ("u8", 256),
        ("u16", 65_536),
        ("u32", 4_294_967_296),
        ("u64", -1),
        ("u128", -1),
    ] {
        let err = render("amount", ty, value).expect_err("out-of-range unsigned value");
        let msg = format!("{err:#}");
        assert!(msg.contains(&format!("`{ty}`")), "missing type: {msg}");
        assert!(msg.contains("amount"), "missing argument name: {msg}");
        assert!(msg.contains(&value.to_string()), "missing value: {msg}");
        assert!(msg.contains("out of range"), "unclear error: {msg}");
    }
}

#[test]
fn out_of_range_signed_integers_are_rejected_before_invocation() {
    for (ty, value) in [
        ("i8", 128),
        ("i8", -129),
        ("i16", 32_768),
        ("i16", -32_769),
        ("i32", 2_147_483_648),
        ("i32", -2_147_483_649),
    ] {
        let err = render("delta", ty, value).expect_err("out-of-range signed value");
        let msg = format!("{err:#}");
        assert!(msg.contains(&format!("`{ty}`")), "missing type: {msg}");
        assert!(msg.contains("delta"), "missing argument name: {msg}");
        assert!(msg.contains(&value.to_string()), "missing value: {msg}");
        assert!(msg.contains("out of range"), "unclear error: {msg}");
    }
}

#[test]
fn boundary_values_are_rendered_verbatim() {
    for (ty, value, expected) in [
        ("u8", 0, "0"),
        ("u8", 255, "255"),
        ("u16", 65_535, "65535"),
        ("u32", 4_294_967_295, "4294967295"),
        ("u64", 9_223_372_036_854_775_807, "9223372036854775807"),
        ("i8", -128, "-128"),
        ("i8", 127, "127"),
        ("i32", -2_147_483_648, "-2147483648"),
        ("i32", 2_147_483_647, "2147483647"),
        ("i64", i64::MIN, "-9223372036854775808"),
        ("i64", i64::MAX, "9223372036854775807"),
    ] {
        assert_eq!(
            render("n", ty, value).unwrap(),
            vec!["--n".to_string(), expected.to_string()],
            "type `{ty}`, value `{value}`"
        );
    }
}

#[test]
fn the_error_names_function_argument_type_and_value() {
    let err = render("fee_bps", "u8", 300).expect_err("300 must not fit in u8");
    let msg = format!("{err:#}");
    for expected in [
        "function `deposit`",
        "argument `fee_bps`",
        "declared type `u8`",
        "300",
    ] {
        assert!(msg.contains(expected), "expected {expected:?} in: {msg}");
    }
}

#[test]
fn non_integer_values_are_not_range_checked() {
    let arg = ArgSpec::Typed(TypedArg {
        name: "n".to_string(),
        ty: "u8".to_string(),
        value: Some(Value::String("99999999999".to_string())),
        generate: false,
    });
    assert_eq!(
        render_args(&[arg], "f").unwrap(),
        vec!["--n", "99999999999"]
    );

    let symbol = ArgSpec::Typed(TypedArg {
        name: "label".to_string(),
        ty: "symbol".to_string(),
        value: Some(Value::Integer(999_999_999_999)),
        generate: false,
    });
    assert_eq!(
        render_args(&[symbol], "f").unwrap(),
        vec!["--label", "999999999999"]
    );
}
