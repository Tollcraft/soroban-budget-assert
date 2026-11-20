//! Integration tests for issue #1025:
//! Validate that `generate = true` is only used on `address` arguments in `arg_spec.rs`,
//! returning an error naming the argument and type if applied elsewhere.

#[path = "../src/arg_spec.rs"]
mod arg_spec;

use arg_spec::{render_args, ArgSpec, TypedArg};

#[test]
fn test_generate_rejected_on_scalar_and_complex_types() {
    let test_cases = vec![
        ("u32", Some(toml::Value::Integer(10))),
        ("i128", Some(toml::Value::String("1000".to_string()))),
        ("bool", Some(toml::Value::Boolean(true))),
        ("string", Some(toml::Value::String("hello".to_string()))),
        ("bytes", Some(toml::Value::String("0x1234".to_string()))),
        ("json", Some(toml::Value::Table(toml::map::Map::new()))),
    ];

    for (ty, val) in test_cases {
        let arg = TypedArg {
            name: format!("arg_{ty}"),
            ty: ty.to_string(),
            value: val,
            generate: true,
        };

        let err = render_args(&[ArgSpec::Typed(arg)], "test_func")
            .expect_err(&format!("expected error for generate=true on type {ty}"));

        let msg = format!("{err:#}");
        assert!(
            msg.contains(&format!("arg_{ty}")),
            "expected error to name argument `arg_{ty}`, got: {msg}"
        );
        assert!(
            msg.contains(ty),
            "expected error to name type `{ty}`, got: {msg}"
        );
        assert!(
            msg.contains("generate = true"),
            "expected error to mention generate flag constraint, got: {msg}"
        );
    }
}

#[test]
fn test_generate_allowed_on_address_type() {
    let arg = TypedArg {
        name: "recipient".to_string(),
        ty: "address".to_string(),
        value: None,
        generate: true,
    };

    let rendered = render_args(&[ArgSpec::Typed(arg)], "transfer")
        .expect("generate = true on address should succeed");

    assert_eq!(rendered.len(), 2);
    assert_eq!(rendered[0], "--recipient");
    assert!(
        rendered[1].starts_with('G'),
        "generated address must be a G... strkey, got {}",
        rendered[1]
    );
}

#[test]
fn test_address_without_generate_requires_value() {
    let arg = TypedArg {
        name: "recipient".to_string(),
        ty: "address".to_string(),
        value: None,
        generate: false,
    };

    assert!(render_args(&[ArgSpec::Typed(arg)], "transfer").is_err());
}
