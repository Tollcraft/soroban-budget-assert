//! Typed function-argument specifications for `budget.toml` (issue #152).
//!
//! `[functions.<name>].args` historically took a flat list of strings passed
//! straight through to `stellar contract invoke` after the `--`:
//!
//! ```toml
//! [functions.do_expensive_work]
//! args = ["--n", "10000"]
//! ```
//!
//! That is fine for a `u32`, but a real entry point taking an address, a
//! symbol, a struct or a vector needs the value constructed. This module adds
//! a typed form that coexists with the flat one — each entry is *either* a
//! bare string (unchanged) *or* a table naming the argument, its type, and
//! its value:
//!
//! ```toml
//! [functions.transfer]
//! args = [
//!   { name = "to", type = "address", generate = true },
//!   { name = "amount", type = "i128", value = "1000000" },
//!   { name = "memo", type = "symbol", value = "topup" },
//! ]
//! ```
//!
//! Every spec renders to the same `--<name> <value>` pair the flat form
//! produced by hand, so the downstream invocation path is unchanged.

use anyhow::{bail, Context};

/// One entry in a function's `args` list: a verbatim string or a typed spec.
#[derive(serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
pub(crate) enum ArgSpec {
    /// Passed through unchanged, e.g. `"--n"` then `"10000"`.
    Raw(String),
    /// A named, typed argument.
    Typed(TypedArg),
}

#[derive(serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct TypedArg {
    /// The parameter name, rendered as `--<name>`.
    pub name: String,
    /// One of the type keywords in [`render_typed`].
    #[serde(rename = "type")]
    pub ty: String,
    /// The literal value. Required for every type except `bool` (defaults
    /// `false`) and `address` when `generate = true`.
    #[serde(default)]
    pub value: Option<toml::Value>,
    /// `address` only: derive a deterministic valid strkey from `name`
    /// instead of taking `value`. Lets a function that needs *an* address
    /// simulate without a checked-in account.
    #[serde(default)]
    pub generate: bool,
}

/// Renders a function's whole `args` list to the flat CLI vector.
pub(crate) fn render_args(specs: &[ArgSpec], function: &str) -> anyhow::Result<Vec<String>> {
    let mut out = Vec::new();
    for spec in specs {
        match spec {
            ArgSpec::Raw(s) => out.push(s.clone()),
            ArgSpec::Typed(arg) => {
                out.push(format!("--{}", arg.name));
                out.push(
                    render_typed(arg).with_context(|| {
                        format!("function `{function}`, argument `{}`", arg.name)
                    })?,
                );
            }
        }
    }
    Ok(out)
}

fn render_typed(arg: &TypedArg) -> anyhow::Result<String> {
    let scalar = || -> anyhow::Result<String> {
        match arg.value.as_ref() {
            Some(toml::Value::String(s)) => Ok(s.clone()),
            Some(toml::Value::Integer(i)) => Ok(i.to_string()),
            Some(toml::Value::Boolean(b)) => Ok(b.to_string()),
            Some(other) => bail!("expected a scalar value, got {}", other.type_str()),
            None => bail!("`value` is required for type `{}`", arg.ty),
        }
    };

    match arg.ty.as_str() {
        "u32" | "i32" | "u64" | "i64" | "u128" | "i128" | "u256" | "i256" | "symbol" | "string" => {
            scalar()
        }
        "bool" => match arg.value.as_ref() {
            None => Ok("false".to_string()),
            Some(toml::Value::Boolean(b)) => Ok(b.to_string()),
            Some(other) => bail!("`bool` value must be a boolean, got {}", other.type_str()),
        },
        "bytes" | "bytesn" => {
            let s = scalar()?;
            let hex = s.strip_prefix("0x").unwrap_or(&s);
            if hex.is_empty() || !hex.bytes().all(|b| b.is_ascii_hexdigit()) || hex.len() % 2 != 0 {
                bail!("`{}` value must be an even-length hex string, got {s:?}", arg.ty);
            }
            Ok(hex.to_string())
        }
        "address" => {
            if arg.generate {
                Ok(generated_address(&arg.name))
            } else {
                let s = scalar()?;
                if !(s.starts_with('G') || s.starts_with('C')) {
                    bail!("`address` value must be a G… or C… strkey, got {s:?}");
                }
                Ok(s)
            }
        }
        // Structs, vectors and maps: the CLI already accepts JSON for these,
        // so take an inline table / array / string and forward it as JSON.
        "json" | "struct" | "vec" | "map" => {
            let v = arg
                .value
                .as_ref()
                .with_context(|| format!("`value` is required for type `{}`", arg.ty))?;
            serde_json::to_string(v).context("serialising the value to JSON for the CLI")
        }
        other => bail!(
            "unknown argument type `{other}` (expected one of: u32/i32/u64/i64/u128/i128/u256/i256, \
             bool, symbol, string, bytes, address, json/struct/vec/map)"
        ),
    }
}

/// A deterministic, valid ed25519 public-key strkey derived from `seed`.
///
/// Not a real account — `--build-only` never touches the network — but a
/// well-formed `G…` address so XDR construction succeeds and two runs of the
/// same `budget.toml` build byte-identical transactions.
fn generated_address(seed: &str) -> String {
    // FNV-1a over the seed, expanded to 32 bytes. No RNG dependency, stable
    // across runs and platforms.
    let mut bytes = [0u8; 32];
    let mut hash: u64 = 0xcbf29ce484222325;
    for (i, slot) in bytes.iter_mut().enumerate() {
        hash ^= seed
            .as_bytes()
            .get(i % seed.len().max(1))
            .copied()
            .unwrap_or(0) as u64;
        hash = hash.wrapping_mul(0x100000001b3);
        hash ^= (i as u64).wrapping_mul(0x9e3779b97f4a7c15);
        *slot = (hash >> ((i % 8) * 8)) as u8;
    }
    format!("{}", stellar_strkey::ed25519::PublicKey(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(toml_str: &str) -> Vec<String> {
        #[derive(serde::Deserialize)]
        struct Wrap {
            args: Vec<ArgSpec>,
        }
        let w: Wrap = toml::from_str(toml_str).unwrap();
        render_args(&w.args, "f").unwrap()
    }

    #[test]
    fn bare_strings_pass_through_unchanged() {
        assert_eq!(typed(r#"args = ["--n", "10000"]"#), vec!["--n", "10000"]);
    }

    #[test]
    fn scalars_render_as_name_value_pairs() {
        assert_eq!(
            typed(
                r#"args = [
                    { name = "n", type = "u32", value = "10000" },
                    { name = "amount", type = "i128", value = 42 },
                    { name = "memo", type = "symbol", value = "topup" },
                ]"#
            ),
            vec!["--n", "10000", "--amount", "42", "--memo", "topup"]
        );
    }

    #[test]
    fn bool_defaults_to_false_without_a_value() {
        assert_eq!(
            typed(r#"args = [{ name = "flag", type = "bool" }]"#),
            vec!["--flag", "false"]
        );
    }

    #[test]
    fn bytes_strips_0x_and_validates_hex() {
        assert_eq!(
            typed(r#"args = [{ name = "salt", type = "bytes", value = "0xDEADBEEF" }]"#),
            vec!["--salt", "DEADBEEF"]
        );
    }

    #[test]
    fn generated_address_is_a_valid_stable_strkey() {
        let a = typed(r#"args = [{ name = "to", type = "address", generate = true }]"#);
        let b = typed(r#"args = [{ name = "to", type = "address", generate = true }]"#);
        assert_eq!(a, b, "generation must be deterministic");
        assert!(a[1].starts_with('G') && a[1].len() == 56);
        assert!(stellar_strkey::ed25519::PublicKey::from_string(&a[1]).is_ok());
    }

    #[test]
    fn json_type_forwards_structured_values() {
        assert_eq!(
            typed(r#"args = [{ name = "cfg", type = "json", value = { a = 1, b = "x" } }]"#),
            vec!["--cfg", r#"{"a":1,"b":"x"}"#]
        );
    }

    #[test]
    fn unknown_type_is_a_named_error_not_a_silent_skip() {
        #[derive(serde::Deserialize)]
        struct Wrap {
            args: Vec<ArgSpec>,
        }
        let w: Wrap =
            toml::from_str(r#"args = [{ name = "x", type = "widget", value = "1" }]"#).unwrap();
        let err = render_args(&w.args, "do_work").unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("widget"), "{msg}");
        assert!(msg.contains("do_work"), "{msg}");
    }

    #[test]
    fn missing_required_value_is_an_error() {
        #[derive(serde::Deserialize)]
        struct Wrap {
            args: Vec<ArgSpec>,
        }
        let w: Wrap = toml::from_str(r#"args = [{ name = "n", type = "u32" }]"#).unwrap();
        assert!(render_args(&w.args, "f").is_err());
    }

    // =================================================================
    // Helpers for the coverage tests below
    // =================================================================

    /// Deserialise a `budget.toml` body exactly as `FunctionConfig.args` does.
    #[derive(serde::Deserialize)]
    struct Doc {
        args: Vec<ArgSpec>,
    }

    fn specs(toml_str: &str) -> Result<Vec<ArgSpec>, toml::de::Error> {
        toml::from_str::<Doc>(toml_str).map(|doc| doc.args)
    }

    /// Render a whole document, for a function literally named `f`.
    fn render(toml_str: &str) -> anyhow::Result<Vec<String>> {
        render_args(&specs(toml_str).expect("the fixture must deserialise"), "f")
    }

    /// The full `{:#}` error chain for a document that must fail to render.
    fn render_err(toml_str: &str) -> String {
        format!(
            "{:#}",
            render(toml_str).expect_err("the fixture must fail to render")
        )
    }

    /// One typed spec on its own: `one(r#"{ name = "n", type = "u32" }"#)`.
    fn one(spec: &str) -> Vec<String> {
        typed(&format!("args = [{spec}]"))
    }

    /// One typed spec that must fail, as its full `{:#}` chain.
    fn one_err(spec: &str) -> String {
        render_err(&format!("args = [{spec}]"))
    }

    /// A valid ed25519 strkey, built rather than pasted so no fixture looks
    /// like a real account.
    fn account(seed: u8) -> String {
        format!("{}", stellar_strkey::ed25519::PublicKey([seed; 32]))
    }

    const SCALAR_TYPES: [&str; 10] = [
        "u32", "i32", "u64", "i64", "u128", "i128", "u256", "i256", "symbol", "string",
    ];
    const BYTES_TYPES: [&str; 2] = ["bytes", "bytesn"];
    const STRUCTURED_TYPES: [&str; 4] = ["json", "struct", "vec", "map"];
    /// The keyword hint appended to every unknown-type error. Kept as one
    /// constant so adding a keyword without updating it fails loudly.
    const EXPECTED_TYPES: &str = "(expected one of: u32/i32/u64/i64/u128/i128/u256/i256, \
                                  bool, symbol, string, bytes, address, json/struct/vec/map)";

    fn unknown_type(ty: &str) -> String {
        format!("function `f`, argument `x`: unknown argument type `{ty}` {EXPECTED_TYPES}")
    }

    // =================================================================
    // Flat (bare-string) entries: the pre-#152 form must be untouched
    // =================================================================

    #[test]
    fn an_empty_args_list_renders_no_arguments() {
        assert_eq!(typed(r#"args = []"#), Vec::<String>::new());
        assert!(render_args(&[], "do_work").unwrap().is_empty());
    }

    #[test]
    fn bare_strings_keep_their_order_and_count() {
        assert_eq!(
            typed(r#"args = ["--n", "10000", "--m", "20", "--flag"]"#),
            vec!["--n", "10000", "--m", "20", "--flag"]
        );
    }

    #[test]
    fn an_empty_bare_string_passes_through_instead_of_being_dropped() {
        // `boundary_tests.rs` feeds `ArgSpec::Raw(String::new())` through this
        // same path; pin that the renderer neither filters nor rewrites it.
        assert_eq!(typed(r#"args = [""]"#), vec![""]);
    }

    #[test]
    fn repeated_bare_strings_are_not_collapsed() {
        assert_eq!(
            typed(r#"args = ["--n", "--n", "1", "1"]"#),
            vec!["--n", "--n", "1", "1"]
        );
    }

    // =================================================================
    // Mixing both forms: ordering is the whole contract
    // =================================================================

    #[test]
    fn mixed_raw_and_typed_entries_interleave_in_declaration_order() {
        assert_eq!(
            typed(
                r#"args = [
                    "--verbose",
                    { name = "n", type = "u32", value = "5" },
                    "trailing",
                    { name = "memo", type = "symbol", value = "x" },
                ]"#
            ),
            vec!["--verbose", "--n", "5", "trailing", "--memo", "x"]
        );
    }

    #[test]
    fn each_typed_spec_contributes_exactly_two_tokens() {
        let list = (0..20)
            .map(|i| format!("{{ name = \"a{i}\", type = \"u32\", value = \"{i}\" }}"))
            .collect::<Vec<_>>()
            .join(", ");
        let out = typed(&format!("args = [{list}]"));
        assert_eq!(out.len(), 40);
        assert_eq!(out[0], "--a0");
        assert_eq!(out[38], "--a19");
        assert_eq!(out[39], "19");
    }

    #[test]
    fn a_typed_spec_renders_the_same_pair_a_flat_list_would_have_produced() {
        // The headline claim of #152: existing `budget.toml` files and the
        // invocation path downstream are unchanged.
        assert_eq!(
            typed(r#"args = ["--n", "10000"]"#),
            typed(r#"args = [{ name = "n", type = "u32", value = "10000" }]"#)
        );
    }

    #[test]
    fn an_empty_argument_name_still_renders_a_bare_flag_marker() {
        // `name` is not validated here; `--` is what the CLI receives.
        assert_eq!(
            one(r#"{ name = "", type = "u32", value = "1" }"#),
            vec!["--", "1"]
        );
    }

    #[test]
    fn an_argument_name_is_used_verbatim_without_trimming() {
        assert_eq!(
            one(r#"{ name = " spaced ", type = "u32", value = "1" }"#),
            vec!["-- spaced ", "1"]
        );
    }

    // =================================================================
    // Scalar types: the ten keywords that forward `scalar()`
    // =================================================================

    #[test]
    fn every_scalar_keyword_forwards_a_string_value_verbatim() {
        for ty in SCALAR_TYPES {
            assert_eq!(
                one(&format!(
                    "{{ name = \"arg\", type = {ty:?}, value = \"12345\" }}"
                )),
                vec!["--arg".to_string(), "12345".to_string()],
                "type `{ty}`"
            );
        }
    }

    #[test]
    fn a_toml_integer_renders_through_its_own_display() {
        for (ty, value, expected) in [
            ("u32", "0", "0"),
            ("i32", "-1", "-1"),
            ("u64", "9223372036854775807", "9223372036854775807"),
            ("i128", "-9223372036854775808", "-9223372036854775808"),
            ("symbol", "42", "42"),
        ] {
            assert_eq!(
                one(&format!(
                    "{{ name = \"n\", type = {ty:?}, value = {value} }}"
                )),
                vec!["--n".to_string(), expected.to_string()],
                "type `{ty}`"
            );
        }
    }

    #[test]
    fn a_value_wider_than_toml_integers_must_be_written_as_a_string() {
        // TOML integers are i64, so a `u128`/`i256` argument can only be given
        // as a string. Pin both halves of that boundary.
        assert!(
            specs(r#"args = [{ name = "x", type = "u128", value = 340282366920938463463374607431768211455 }]"#)
                .is_err(),
            "an out-of-i64-range literal must not deserialise"
        );
        assert_eq!(
            one(
                r#"{ name = "x", type = "u128", value = "340282366920938463463374607431768211455" }"#
            ),
            vec!["--x", "340282366920938463463374607431768211455"]
        );
    }

    #[test]
    fn a_boolean_value_renders_for_a_scalar_type() {
        // `scalar()` accepts any scalar, so `value = true` on a `u32` renders
        // "true" rather than being type-checked. Rejected later, by the CLI.
        assert_eq!(
            one(r#"{ name = "n", type = "u32", value = true }"#),
            vec!["--n", "true"]
        );
    }

    #[test]
    fn numeric_range_and_symbol_shape_are_not_validated_here() {
        assert_eq!(
            one(r#"{ name = "n", type = "u32", value = "99999999999" }"#),
            vec!["--n", "99999999999"]
        );
        assert_eq!(
            one(r#"{ name = "s", type = "symbol", value = "not a valid symbol!" }"#),
            vec!["--s", "not a valid symbol!"]
        );
    }

    #[test]
    fn a_long_string_value_is_not_truncated() {
        let long = "x".repeat(10_000);
        let out = one(&format!(
            "{{ name = \"s\", type = \"string\", value = {long:?} }}"
        ));
        assert_eq!(out[0], "--s");
        assert_eq!(out[1].len(), 10_000);
    }

    #[test]
    fn a_value_containing_whitespace_or_newlines_stays_one_token() {
        // The rendered list is an argv vector, never a shell string, so spaces
        // need no escaping and must survive intact.
        assert_eq!(
            one(r#"{ name = "s", type = "string", value = "two words" }"#),
            vec!["--s", "two words"]
        );
        assert_eq!(
            one(r#"{ name = "s", type = "string", value = "line\nbreak" }"#),
            vec!["--s", "line\nbreak"]
        );
    }

    #[test]
    fn a_value_that_looks_like_a_flag_is_passed_through_unescaped() {
        // Known sharp edge, pinned deliberately: `["--n", "--m"]` makes the CLI
        // read the second one as another option. Nothing in this module quotes
        // or rejects it, so a `budget.toml` author has to avoid it.
        assert_eq!(
            one(r#"{ name = "n", type = "u32", value = "--m" }"#),
            vec!["--n", "--m"]
        );
    }

    // =================================================================
    // Scalar types: error states
    // =================================================================

    #[test]
    fn a_non_scalar_value_names_the_toml_type_it_got() {
        for (value, type_str) in [
            ("[1, 2]", "array"),
            ("{ a = 1 }", "table"),
            ("1.5", "float"),
            ("1979-05-27T07:32:00Z", "datetime"),
        ] {
            assert_eq!(
                one_err(&format!(
                    "{{ name = \"n\", type = \"u32\", value = {value} }}"
                )),
                format!("function `f`, argument `n`: expected a scalar value, got {type_str}"),
                "value `{value}`"
            );
        }
    }

    #[test]
    fn every_scalar_type_without_a_value_says_which_type() {
        for ty in SCALAR_TYPES {
            assert_eq!(
                one_err(&format!("{{ name = \"n\", type = {ty:?} }}")),
                format!("function `f`, argument `n`: `value` is required for type `{ty}`"),
            );
        }
    }

    // =================================================================
    // bool
    // =================================================================

    #[test]
    fn bool_renders_an_explicit_true_or_false() {
        assert_eq!(
            one(r#"{ name = "flag", type = "bool", value = true }"#),
            vec!["--flag", "true"]
        );
        assert_eq!(
            one(r#"{ name = "flag", type = "bool", value = false }"#),
            vec!["--flag", "false"]
        );
    }

    #[test]
    fn a_bool_value_must_actually_be_a_boolean() {
        for (value, type_str) in [
            ("\"true\"", "string"),
            ("1", "integer"),
            ("1.0", "float"),
            ("[]", "array"),
            ("{}", "table"),
            ("1979-05-27T07:32:00Z", "datetime"),
        ] {
            assert_eq!(
                one_err(&format!(
                    "{{ name = \"flag\", type = \"bool\", value = {value} }}"
                )),
                format!(
                    "function `f`, argument `flag`: `bool` value must be a boolean, got {type_str}"
                ),
                "value `{value}`"
            );
        }
    }

    // =================================================================
    // bytes / bytesn
    // =================================================================

    #[test]
    fn bytes_strips_the_prefix_and_preserves_letter_case() {
        for ty in BYTES_TYPES {
            assert_eq!(
                one(&format!(
                    "{{ name = \"salt\", type = {ty:?}, value = \"0xDEADBEEF\" }}"
                )),
                vec!["--salt".to_string(), "DEADBEEF".to_string()],
                "type `{ty}`"
            );
            assert_eq!(
                one(&format!(
                    "{{ name = \"salt\", type = {ty:?}, value = \"0xdeadbeef\" }}"
                )),
                vec!["--salt".to_string(), "deadbeef".to_string()],
                "type `{ty}`"
            );
        }
    }

    #[test]
    fn bytes_accepts_hex_that_has_no_prefix() {
        for ty in BYTES_TYPES {
            assert_eq!(
                one(&format!(
                    "{{ name = \"salt\", type = {ty:?}, value = \"ff00\" }}"
                )),
                vec!["--salt".to_string(), "ff00".to_string()]
            );
        }
    }

    #[test]
    fn bytes_rejects_malformed_hex_and_quotes_the_offending_value() {
        for (ty, value) in [
            ("bytes", "\"\""),
            ("bytes", "\"0x\""),
            ("bytesn", "\"0x\""),
            ("bytes", "\"0xABC\""),
            ("bytesn", "\"123\""),
            ("bytes", "\"0xGG\""),
            ("bytes", "\"0xzz\""),
            ("bytes", "\"0xde ad\""),
            ("bytes", "\"0x1234 5678\""),
            ("bytes", "\"0x12;\""),
        ] {
            let err = one_err(&format!(
                "{{ name = \"salt\", type = {ty:?}, value = {value} }}"
            ));
            // `value` is debug-quoted, and the *declared* keyword is named.
            assert_eq!(
                err,
                format!("function `f`, argument `salt`: `{ty}` value must be an even-length hex string, got {value}"),
                "type `{ty}`, value `{value}`"
            );
        }
    }

    #[test]
    fn bytes_checks_the_rendered_text_not_the_number() {
        // An integer value is fine when every digit is a hex digit and the
        // text has even length; 1234 renders, 123 does not.
        assert_eq!(
            one(r#"{ name = "salt", type = "bytes", value = 1234 }"#),
            vec!["--salt", "1234"]
        );
        assert!(
            one_err(r#"{ name = "salt", type = "bytes", value = 123 }"#).contains("even-length")
        );
    }

    #[test]
    fn bytesn_is_not_size_checked_against_its_width() {
        // `bytesn` shares the `bytes` arm: nothing here knows N, so a 1-byte
        // value for a 32-byte field renders and fails at the CLI instead.
        assert_eq!(
            one(r#"{ name = "k", type = "bytesn", value = "0x00" }"#),
            vec!["--k", "00"]
        );
    }

    #[test]
    fn a_full_width_bytes_value_renders() {
        let hex = "0x".to_string() + &"ab".repeat(32);
        let out = one(&format!(
            "{{ name = \"k\", type = \"bytes\", value = {hex:?} }}"
        ));
        assert_eq!(out[1].len(), 64);
    }

    // =================================================================
    // address: explicit values
    // =================================================================

    #[test]
    fn address_accepts_a_valid_account_strkey_verbatim() {
        let addr = account(0);
        assert!(addr.starts_with('G'), "{addr}");
        assert_eq!(
            one(&format!(
                "{{ name = \"to\", type = \"address\", value = {addr:?} }}"
            )),
            vec!["--to".to_string(), addr]
        );
    }

    #[test]
    fn address_accepts_a_contract_strkey() {
        let contract = format!("{}", stellar_strkey::Contract([1u8; 32]));
        assert!(contract.starts_with('C'), "{contract}");
        assert_eq!(
            one(&format!(
                "{{ name = \"wasm\", type = \"address\", value = {contract:?} }}"
            )),
            vec!["--wasm".to_string(), contract]
        );
    }

    #[test]
    fn address_rejects_anything_that_is_not_g_or_c() {
        for value in ["", "0", "1x", "SBD:", "g", "c"] {
            // a lone `G`/`C` passes (see the next test); these do not.
            let err = one_err(&format!(
                "{{ name = \"to\", type = \"address\", value = {value:?} }}"
            ));
            assert_eq!(
                err,
                format!("function `f`, argument `to`: `address` value must be a G… or C… strkey, got {value:?}"),
                "value `{value}`"
            );
        }
    }

    #[test]
    fn a_muxed_account_strkey_is_rejected_by_the_prefix_rule() {
        // `M…` is a valid strkey but not an invoke-able address here, so the
        // two-character rule catches it. Pin it so widening the rule is a
        // deliberate change rather than an accident.
        assert!(
            one_err(r#"{ name = "to", type = "address", value = "MA7QYNF7SOWQNLGL" }"#)
                .contains("G… or C… strkey")
        );
    }

    #[test]
    fn address_is_checked_by_prefix_only() {
        // Full strkey decoding happens when the CLI builds the XDR. A `G` that
        // is not a real key therefore renders here.
        assert_eq!(
            one(r#"{ name = "to", type = "address", value = "Gnot-a-real-key" }"#),
            vec!["--to", "Gnot-a-real-key"]
        );
    }

    // =================================================================
    // address: generate = true
    // =================================================================

    #[test]
    fn generate_derives_a_stable_address_from_the_argument_name() {
        // Structural assertions only: the derivation is an internal detail, so
        // pinning a literal strkey would break on any algorithm change.
        let to = one(r#"{ name = "to", type = "address", generate = true }"#);
        let again = one(r#"{ name = "to", type = "address", generate = true }"#);
        let from = one(r#"{ name = "from", type = "address", generate = true }"#);
        assert_eq!(to, again, "generation must be deterministic");
        assert_ne!(to, from, "distinct names must not collide");
        for addr in [&to[1], &from[1]] {
            assert!(addr.starts_with('G') && addr.len() == 56, "{addr}");
            assert!(
                stellar_strkey::ed25519::PublicKey::from_string(addr).is_ok(),
                "{addr}"
            );
        }
    }

    #[test]
    fn generate_wins_over_an_explicit_value() {
        // Branch order matters: a bad `value` alongside `generate = true` is
        // ignored rather than validated.
        let out = one(r#"{ name = "to", type = "address", value = "nonsense", generate = true }"#);
        assert_eq!(out[0], "--to");
        assert!(stellar_strkey::ed25519::PublicKey::from_string(&out[1]).is_ok());
    }

    #[test]
    fn generate_false_takes_the_provided_value() {
        let addr = account(2);
        assert_eq!(
            one(&format!(
                "{{ name = \"to\", type = \"address\", value = {addr:?}, generate = false }}"
            )),
            vec!["--to".to_string(), addr]
        );
    }

    #[test]
    fn an_empty_argument_name_generates_without_panicking() {
        // Covers the `seed.len().max(1)` guard: an empty seed must not divide
        // or index out of bounds.
        let out = one(r#"{ name = "", type = "address", generate = true }"#);
        assert_eq!(out[0], "--");
        assert!(stellar_strkey::ed25519::PublicKey::from_string(&out[1]).is_ok());
    }

    #[test]
    fn generated_address_is_stable_and_valid_at_every_seed_length() {
        let long_31 = "z".repeat(31);
        let long_32 = "z".repeat(32);
        let long_33 = "z".repeat(33);
        let huge = "s".repeat(1_000);
        let empty = String::new();
        for seed in [
            &empty,
            &"a".to_string(),
            &"ab".to_string(),
            &long_31,
            &long_32,
            &long_33,
            &huge,
        ] {
            let direct = generated_address(seed);
            assert_eq!(direct, generated_address(seed), "seed {seed:?}");
            assert!(
                stellar_strkey::ed25519::PublicKey::from_string(&direct).is_ok(),
                "seed {seed:?}"
            );
        }
    }

    #[test]
    fn generated_address_reads_the_name_through_a_repeating_window() {
        // KNOWN LIMITATION, pinned rather than fixed (out of #696's scope): the
        // seed is read as `seed[i % seed.len()]`, so a name only influences the
        // hash through that window. Two names can therefore derive the *same*
        // dummy address, which matters if a `budget.toml` expects `from` and
        // `to` to differ. Distinct realistic argument names do not collide
        // (see `generated_addresses_are_distinct_across_many_names`).
        assert_eq!(
            generated_address("z".repeat(31).as_str()),
            generated_address(&"z".repeat(32))
        );
        assert_eq!(generated_address("ab"), generated_address("abab"));
        let prefix = "x".repeat(32);
        assert_eq!(
            generated_address(&(prefix.clone() + "AAA")),
            generated_address(&(prefix + "BBB")),
            "names at or beyond 32 bytes only differ in what the window reads"
        );
    }

    #[test]
    fn a_multibyte_name_generates_without_a_char_boundary_panic() {
        // The seed is walked as bytes, so non-ASCII names are well-defined.
        for name in ["ünïcode", "名前", "wave🌊"] {
            let out = one(&format!(
                "{{ name = {name:?}, type = \"address\", generate = true }}"
            ));
            assert!(
                stellar_strkey::ed25519::PublicKey::from_string(&out[1]).is_ok(),
                "name {name}"
            );
        }
    }

    #[test]
    fn generated_addresses_are_distinct_across_many_names() {
        let mut seen = std::collections::HashSet::new();
        for i in 0..256 {
            assert!(
                seen.insert(generated_address(&format!("arg{i}"))),
                "duplicate address at arg{i}"
            );
        }
    }

    #[test]
    fn generation_is_stateless_across_a_list_of_specs() {
        // The same name in two different documents must render the same key:
        // two runs of one `budget.toml` build byte-identical transactions.
        let solo = one(r#"{ name = "to", type = "address", generate = true }"#);
        let pair = typed(
            r#"args = [
                { name = "amount", type = "i128", value = "1" },
                { name = "to", type = "address", generate = true },
            ]"#,
        );
        assert_eq!(pair[3], solo[1]);
    }

    // =================================================================
    // json / struct / vec / map
    // =================================================================

    #[test]
    fn every_structured_keyword_forwards_its_value_as_json() {
        for ty in STRUCTURED_TYPES {
            assert_eq!(
                one(&format!(
                    "{{ name = \"cfg\", type = {ty:?}, value = [1, 2] }}"
                )),
                vec!["--cfg".to_string(), "[1,2]".to_string()],
                "type `{ty}`"
            );
        }
    }

    #[test]
    fn structured_keywords_render_each_toml_type_as_json() {
        for (value, expected) in [
            ("\"x\"", "\"x\""),
            ("1", "1"),
            ("1.5", "1.5"),
            ("true", "true"),
            ("[]", "[]"),
            ("{}", "{}"),
            ("[[1], [2]]", "[[1],[2]]"),
            ("[\"a\", \"b\"]", "[\"a\",\"b\"]"),
        ] {
            assert_eq!(
                one(&format!(
                    "{{ name = \"v\", type = \"json\", value = {value} }}"
                )),
                vec!["--v".to_string(), expected.to_string()],
                "value `{value}`"
            );
        }
    }

    #[test]
    fn a_nested_table_becomes_a_json_object() {
        // Compared by reparsing: TOML map iteration order is not part of the
        // contract, the JSON *content* is.
        let out = one(
            r#"{ name = "cfg", type = "struct", value = { alpha = 1, beta = "x", gamma = [true, false] } }"#,
        );
        let parsed: serde_json::Value = serde_json::from_str(&out[1]).unwrap();
        assert_eq!(
            parsed,
            serde_json::json!({ "alpha": 1, "beta": "x", "gamma": [true, false] })
        );
    }

    #[test]
    fn a_toml_datetime_forwards_as_tomls_private_json_shape() {
        // KNOWN GAP, pinned rather than fixed (out of #696's scope):
        // `toml::Value::Datetime` reaches serde's generic serialiser, which
        // encodes it with its tagged private representation instead of a plain
        // string. A `json`/`struct` value written as a TOML datetime therefore
        // arrives at the CLI as the object below. If this test starts failing
        // because the value renders as `"1979-05-27T07:32:00Z"`, that is the
        // fix landing — update the pin, not the expectation.
        let out = one(r#"{ name = "v", type = "json", value = 1979-05-27T07:32:00Z }"#);
        assert_eq!(
            out[1],
            r#"{"$__toml_private_datetime":"1979-05-27T07:32:00Z"}"#
        );
    }

    #[test]
    fn every_structured_keyword_requires_a_value() {
        // This path is a `with_context`, not a `bail!`, so it is pinned apart
        // from the scalar `value` is required message.
        for ty in STRUCTURED_TYPES {
            assert_eq!(
                one_err(&format!("{{ name = \"cfg\", type = {ty:?} }}")),
                format!("function `f`, argument `cfg`: `value` is required for type `{ty}`"),
            );
        }
    }

    // =================================================================
    // Unknown / mistyped type keywords
    // =================================================================

    #[test]
    fn an_unknown_type_is_a_named_error_listing_the_accepted_keywords() {
        assert_eq!(
            one_err(r#"{ name = "x", type = "widget", value = "1" }"#),
            unknown_type("widget")
        );
    }

    #[test]
    fn type_keywords_are_case_sensitive_and_untrimmed() {
        for ty in [
            "U32", "u32 ", " u32", "\tu32", "uint", "i32 ", "U64", "I128", "Address", "Bytes",
            "Json", "String", "boolx", "address2", "symbols", "", " ",
        ] {
            assert_eq!(
                one_err(&format!("{{ name = \"x\", type = {ty:?} }}")),
                unknown_type(ty),
                "type {ty:?}"
            );
        }
    }

    #[test]
    fn a_missing_value_error_keeps_the_unknown_type_as_the_root_cause() {
        let parsed = specs(r#"args = [{ name = "amount", type = "widget" }]"#).unwrap();
        let err = render_args(&parsed, "deposit").unwrap_err();
        assert_eq!(
            format!("{err:#}"),
            format!("function `deposit`, argument `amount`: unknown argument type `widget` {EXPECTED_TYPES}")
        );
        assert_eq!(
            err.root_cause().to_string(),
            format!("unknown argument type `widget` {EXPECTED_TYPES}")
        );
    }

    // =================================================================
    // Error context: what a user sees when a list is partly wrong
    // =================================================================

    #[test]
    fn one_failing_spec_aborts_the_whole_list() {
        // No partially-rendered vector escapes to the caller.
        assert!(render(r#"args = ["--ok", { name = "bad", type = "widget" }]"#).is_err());
    }

    #[test]
    fn the_error_names_the_function_that_failed() {
        let specs = specs(r#"args = [{ name = "n", type = "u32" }]"#).unwrap();
        for function in ["deposit", "swap_exact_out", "do_expensive_work"] {
            let chain = format!("{:#}", render_args(&specs, function).unwrap_err());
            assert!(
                chain.starts_with(&format!("function `{function}`, argument `n`: ")),
                "{chain}"
            );
        }
    }

    #[test]
    fn the_error_names_the_argument_not_its_position() {
        let chain = render_err(
            r#"args = [
                { name = "first", type = "u32", value = "1" },
                { name = "second", type = "u32", value = [1] },
            ]"#,
        );
        assert!(chain.contains("argument `second`"), "{chain}");
        assert!(!chain.contains("argument `first`"), "{chain}");
    }

    // =================================================================
    // Deserialisation surface: the untagged enum and deny_unknown_fields
    // =================================================================

    #[test]
    fn a_bare_string_is_raw_and_a_table_is_typed() {
        let parsed = specs(r#"args = ["--n", { name = "m", type = "u32", value = "1" }]"#).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0], ArgSpec::Raw("--n".to_string()));
        assert_eq!(
            parsed[1],
            ArgSpec::Typed(TypedArg {
                name: "m".to_string(),
                ty: "u32".to_string(),
                value: Some(toml::Value::String("1".to_string())),
                generate: false,
            })
        );
    }

    #[test]
    fn an_unrecognised_key_is_an_error_not_an_ignored_flag() {
        // `TypedArg` is `deny_unknown_fields`, so `generete = true` cannot
        // silently fall back to a literal `value` read. Serde's untagged
        // representation hides the reason, which the message pin records.
        let err = specs(r#"args = [{ name = "to", type = "address", generete = true }]"#)
            .unwrap_err()
            .to_string();
        assert!(err.contains("untagged enum ArgSpec"), "{err}");
    }

    #[test]
    fn a_typed_spec_needs_both_name_and_type() {
        for body in [
            r#"{ type = "u32", value = "1" }"#,
            r#"{ name = "n", value = "1" }"#,
            r#"{ name = "n", type = "u32", unexpected = 1 }"#,
            r#"{ }"#,
        ] {
            assert!(
                specs(&format!("args = [{body}]")).is_err(),
                "spec {body} must be rejected"
            );
        }
    }

    #[test]
    fn generate_must_be_a_boolean() {
        assert!(specs(r#"args = [{ name = "to", type = "address", generate = "yes" }]"#).is_err());
        assert!(specs(r#"args = [{ name = "to", type = "address", generate = 1 }]"#).is_err());
    }

    #[test]
    fn a_non_string_bare_entry_is_rejected() {
        // `Raw` takes a string only; a loose `1` must not become "--1".
        assert!(specs("args = [1]").is_err());
        assert!(specs("args = [1.5]").is_err());
        assert!(specs("args = [true]").is_err());
        assert!(specs("args = [[1]]").is_err());
    }

    #[test]
    fn value_deserialises_from_every_toml_type() {
        // Rendering narrows the accepted types; deserialisation does not.
        for value in [
            "\"s\"",
            "1",
            "1.5",
            "true",
            "[1]",
            "{ a = 1 }",
            "1979-05-27T07:32:00Z",
            "0x1f",
            "1_000",
            "\"\"\"block\"\"\"",
        ] {
            assert!(
                specs(&format!(
                    "args = [{{ name = \"n\", type = \"u32\", value = {value} }}]"
                ))
                .is_ok(),
                "value {value}"
            );
        }
    }

    #[test]
    fn an_omitted_value_deserialises_to_none() {
        let parsed = specs(r#"args = [{ name = "n", type = "u32" }]"#).unwrap();
        let ArgSpec::Typed(arg) = &parsed[0] else {
            panic!("expected a typed spec");
        };
        assert_eq!(arg.value, None);
        assert!(!arg.generate);
    }

    // =================================================================
    // Whole-document regression: the documented `transfer` example
    // =================================================================

    #[test]
    fn the_documented_transfer_example_renders_a_full_invocation() {
        // Mirrors the `[functions.transfer]` block in the module docs and
        // `docs/src/reference.md`, so the documented example stays true.
        let out = typed(
            r#"args = [
                { name = "to", type = "address", generate = true },
                { name = "amount", type = "i128", value = "1000000" },
                { name = "memo", type = "symbol", value = "topup" },
            ]"#,
        );
        assert_eq!(out.len(), 6);
        assert_eq!(&out[0], "--to");
        assert!(stellar_strkey::ed25519::PublicKey::from_string(&out[1]).is_ok());
        assert_eq!(&out[2], "--amount");
        assert_eq!(&out[3], "1000000");
        assert_eq!(&out[4], "--memo");
        assert_eq!(&out[5], "topup");
    }
}
