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
}
