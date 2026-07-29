use serde::Deserialize;
use std::collections::BTreeMap;

/// A typed function argument value for Soroban contract invocation.
///
/// Supports all Soroban value types: Address, Symbol, String, Bool,
/// integer widths (u32/i32/u64/i64/u128/i128), Bytes, Vec, and Map.
///
/// Nested structures are supported via [`TypedArg::Vec`] and
/// [`TypedArg::Map`].
#[derive(Debug, Clone)]
pub enum TypedArg {
    /// A Stellar account or contract address (G..., C..., or named identity).
    Address(String),
    /// A Soroban symbol.
    Symbol(String),
    /// A string value.
    String(String),
    /// A boolean value.
    Bool(bool),
    /// A 32-bit unsigned integer.
    U32(String),
    /// A 32-bit signed integer.
    I32(String),
    /// A 64-bit unsigned integer.
    U64(String),
    /// A 64-bit signed integer.
    I64(String),
    /// A 128-bit unsigned integer (must be a string in TOML for precision).
    U128(String),
    /// A 128-bit signed integer.
    I128(String),
    /// Raw bytes as a hex string (e.g. `"0102ff"`).
    Bytes(String),
    /// A vector (array) of typed values.
    Vec(Vec<TypedArg>),
    /// A map from string keys to typed values.
    Map(BTreeMap<String, TypedArg>),
}

/// Represents the `args` field in `budget.toml`, supporting both the
/// legacy flat-string format and the new typed format.
#[derive(Debug, Clone)]
pub enum ArgSpec {
    /// Legacy format: `args = ["--flag", "value"]`
    Legacy(Vec<String>),
    /// Typed format: `args = [{ address = "alice" }, { i128 = "1000" }]`
    Typed(Vec<TypedArg>),
}

impl Default for ArgSpec {
    fn default() -> Self {
        ArgSpec::Legacy(Vec::new())
    }
}

impl From<Vec<String>> for ArgSpec {
    fn from(args: Vec<String>) -> Self {
        ArgSpec::Legacy(args)
    }
}

impl From<Vec<&str>> for ArgSpec {
    fn from(args: Vec<&str>) -> Self {
        ArgSpec::Legacy(args.into_iter().map(|s| s.to_string()).collect())
    }
}

impl ArgSpec {
    /// Returns `true` if no arguments are configured.
    pub fn is_empty(&self) -> bool {
        match self {
            ArgSpec::Legacy(args) => args.is_empty(),
            ArgSpec::Typed(args) => args.is_empty(),
        }
    }

    /// Encodes the arguments into a flat `Vec<String>` suitable for
    /// passing to `stellar contract invoke`.
    ///
    /// Legacy args are returned as-is. Typed args are encoded into their
    /// CLI string representation:
    /// - Simple types: the value as a string
    /// - Vec/Map: JSON-encoded
    pub fn encode(&self) -> Vec<String> {
        match self {
            ArgSpec::Legacy(args) => args.clone(),
            ArgSpec::Typed(args) => args.iter().map(|a| a.encode()).collect(),
        }
    }
}

// ── Serde deserialization ──────────────────────────────────────────────

/// Internal helper for untagged deserialization of typed args.
#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
enum RawTypedArg {
    Address { address: String },
    Symbol { symbol: String },
    StringVal { string: String },
    Bool { bool: bool },
    U32 { u32: NumStr },
    I32 { i32: NumStr },
    U64 { u64: NumStr },
    I64 { i64: NumStr },
    U128 { u128: String },
    I128 { i128: String },
    Bytes { bytes: String },
    Vec { vec: Vec<RawTypedArg> },
    Map { map: BTreeMap<String, RawTypedArg> },
}

/// Accepts a TOML integer or string.
#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
enum NumStr {
    Num(i64),
    Str(String),
}

impl From<RawTypedArg> for TypedArg {
    fn from(raw: RawTypedArg) -> Self {
        match raw {
            RawTypedArg::Address { address } => TypedArg::Address(address),
            RawTypedArg::Symbol { symbol } => TypedArg::Symbol(symbol),
            RawTypedArg::StringVal { string } => TypedArg::String(string),
            RawTypedArg::Bool { bool: b } => TypedArg::Bool(b),
            RawTypedArg::U32 { u32 } => TypedArg::U32(numstr_to_string(u32)),
            RawTypedArg::I32 { i32 } => TypedArg::I32(numstr_to_string(i32)),
            RawTypedArg::U64 { u64 } => TypedArg::U64(numstr_to_string(u64)),
            RawTypedArg::I64 { i64 } => TypedArg::I64(numstr_to_string(i64)),
            RawTypedArg::U128 { u128 } => TypedArg::U128(u128),
            RawTypedArg::I128 { i128 } => TypedArg::I128(i128),
            RawTypedArg::Bytes { bytes } => TypedArg::Bytes(bytes),
            RawTypedArg::Vec { vec } => TypedArg::Vec(vec.into_iter().map(|r| r.into()).collect()),
            RawTypedArg::Map { map } => {
                TypedArg::Map(map.into_iter().map(|(k, v)| (k, v.into())).collect())
            }
        }
    }
}

fn numstr_to_string(ns: NumStr) -> String {
    match ns {
        NumStr::Num(n) => n.to_string(),
        NumStr::Str(s) => s,
    }
}

impl<'de> Deserialize<'de> for ArgSpec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Use an untagged helper to try legacy format first, then typed.
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum RawArgSpec {
            Legacy(Vec<String>),
            Typed(Vec<RawTypedArg>),
        }

        let raw = RawArgSpec::deserialize(deserializer)?;
        match raw {
            RawArgSpec::Legacy(args) => Ok(ArgSpec::Legacy(args)),
            RawArgSpec::Typed(args) => {
                Ok(ArgSpec::Typed(args.into_iter().map(|r| r.into()).collect()))
            }
        }
    }
}

// ── Encoding ────────────────────────────────────────────────────────────

impl TypedArg {
    /// Encodes this typed argument to the string format expected by
    /// `stellar contract invoke`.
    ///
    /// Simple scalar types are returned as their string representation.
    /// Complex types (Vec, Map) are JSON-encoded.
    pub fn encode(&self) -> String {
        match self {
            TypedArg::Address(s)
            | TypedArg::Symbol(s)
            | TypedArg::String(s)
            | TypedArg::U32(s)
            | TypedArg::I32(s)
            | TypedArg::U64(s)
            | TypedArg::I64(s)
            | TypedArg::U128(s)
            | TypedArg::I128(s)
            | TypedArg::Bytes(s) => s.clone(),
            TypedArg::Bool(b) => b.to_string(),
            TypedArg::Vec(items) => {
                let values: Vec<serde_json::Value> =
                    items.iter().map(|i| i.to_json_value()).collect();
                serde_json::to_string(&values).unwrap_or_else(|_| "[]".to_string())
            }
            TypedArg::Map(entries) => {
                let map: serde_json::Map<String, serde_json::Value> = entries
                    .iter()
                    .map(|(k, v)| (k.clone(), v.to_json_value()))
                    .collect();
                serde_json::to_string(&map).unwrap_or_else(|_| "{}".to_string())
            }
        }
    }

    /// Converts this typed argument to a `serde_json::Value`, preserving
    /// the structure for complex types.
    fn to_json_value(&self) -> serde_json::Value {
        match self {
            TypedArg::Address(s)
            | TypedArg::Symbol(s)
            | TypedArg::String(s)
            | TypedArg::U32(s)
            | TypedArg::I32(s)
            | TypedArg::U64(s)
            | TypedArg::I64(s)
            | TypedArg::U128(s)
            | TypedArg::I128(s)
            | TypedArg::Bytes(s) => serde_json::Value::String(s.clone()),
            TypedArg::Bool(b) => serde_json::Value::Bool(*b),
            TypedArg::Vec(items) => {
                serde_json::Value::Array(items.iter().map(|i| i.to_json_value()).collect())
            }
            TypedArg::Map(entries) => {
                let map: serde_json::Map<String, serde_json::Value> = entries
                    .iter()
                    .map(|(k, v)| (k.clone(), v.to_json_value()))
                    .collect();
                serde_json::Value::Object(map)
            }
        }
    }
}

/// Encodes a slice of typed arguments into a flat string vector.
pub fn encode_typed_args(args: &[TypedArg]) -> Vec<String> {
    args.iter().map(|a| a.encode()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── ArgSpec deserialization ──────────────────────────────────────────

    #[test]
    fn deserialize_legacy_string_args() {
        let toml_str = r#"
args = ["--n", "10000"]
"#;
        #[derive(Deserialize)]
        struct TestConfig {
            args: ArgSpec,
        }
        let config: TestConfig = toml::from_str(toml_str).unwrap();
        match &config.args {
            ArgSpec::Legacy(args) => {
                assert_eq!(args, &vec!["--n".to_string(), "10000".to_string()]);
            }
            ArgSpec::Typed(_) => panic!("Expected Legacy variant"),
        }
        assert_eq!(
            config.args.encode(),
            vec!["--n".to_string(), "10000".to_string()]
        );
    }

    #[test]
    fn deserialize_typed_simple_args() {
        let toml_str = r#"
args = [
  { address = "alice" },
  { i128 = "1000" },
  { bool = true },
  { string = "hello" },
]
"#;
        #[derive(Deserialize)]
        struct TestConfig {
            args: ArgSpec,
        }
        let config: TestConfig = toml::from_str(toml_str).unwrap();
        match &config.args {
            ArgSpec::Typed(args) => {
                assert_eq!(args.len(), 4);
                assert!(matches!(args[0], TypedArg::Address(_)));
                assert!(matches!(args[1], TypedArg::I128(_)));
                assert!(matches!(args[2], TypedArg::Bool(_)));
                assert!(matches!(args[3], TypedArg::String(_)));
            }
            ArgSpec::Legacy(_) => panic!("Expected Typed variant"),
        }
        let encoded = config.args.encode();
        assert_eq!(encoded.len(), 4);
        assert_eq!(encoded[0], "alice");
        assert_eq!(encoded[1], "1000");
        assert_eq!(encoded[2], "true");
        assert_eq!(encoded[3], "hello");
    }

    #[test]
    fn deserialize_empty_args_legacy() {
        // Empty array defaults to Legacy variant.
        let toml_str = r#"
args = []
"#;
        #[derive(Deserialize)]
        struct TestConfig {
            args: ArgSpec,
        }
        let config: TestConfig = toml::from_str(toml_str).unwrap();
        assert!(config.args.is_empty());
        match config.args {
            ArgSpec::Legacy(ref args) => assert!(args.is_empty()),
            _ => panic!("Expected Legacy for empty args"),
        }
    }

    #[test]
    fn deserialize_args_not_present_defaults_empty() {
        #[derive(Deserialize)]
        struct TestConfig {
            #[serde(default)]
            args: ArgSpec,
        }
        let toml_str = r#"
other_field = "value"
"#;
        let config: TestConfig = toml::from_str(toml_str).unwrap();
        assert!(config.args.is_empty());
    }

    // ── TypedArg encoding ────────────────────────────────────────────────

    #[test]
    fn encode_address() {
        let arg = TypedArg::Address(
            "GCXQH4JQN6J3JHXJ6QZ5W4KQ5X6Y7Z8A9B0C1D2E3F4G5H6I7J8K9L0M".to_string(),
        );
        assert_eq!(
            arg.encode(),
            "GCXQH4JQN6J3JHXJ6QZ5W4KQ5X6Y7Z8A9B0C1D2E3F4G5H6I7J8K9L0M"
        );
    }

    #[test]
    fn encode_symbol() {
        let arg = TypedArg::Symbol("hello".to_string());
        assert_eq!(arg.encode(), "hello");
    }

    #[test]
    fn encode_string() {
        let arg = TypedArg::String("hello world".to_string());
        assert_eq!(arg.encode(), "hello world");
    }

    #[test]
    fn encode_bool_true() {
        assert_eq!(TypedArg::Bool(true).encode(), "true");
    }

    #[test]
    fn encode_bool_false() {
        assert_eq!(TypedArg::Bool(false).encode(), "false");
    }

    #[test]
    fn encode_u32() {
        assert_eq!(TypedArg::U32("42".to_string()).encode(), "42");
    }

    #[test]
    fn encode_i128() {
        assert_eq!(
            TypedArg::I128("-1234567890123456789".to_string()).encode(),
            "-1234567890123456789"
        );
    }

    #[test]
    fn encode_bytes() {
        assert_eq!(TypedArg::Bytes("0102ff".to_string()).encode(), "0102ff");
    }

    #[test]
    fn encode_vec_of_u32() {
        let arg = TypedArg::Vec(vec![
            TypedArg::U32("1".to_string()),
            TypedArg::U32("2".to_string()),
            TypedArg::U32("3".to_string()),
        ]);
        assert_eq!(arg.encode(), r#"["1","2","3"]"#);
    }

    #[test]
    fn encode_vec_of_addresses() {
        let arg = TypedArg::Vec(vec![
            TypedArg::Address("alice".to_string()),
            TypedArg::Address("bob".to_string()),
        ]);
        assert_eq!(arg.encode(), r#"["alice","bob"]"#);
    }

    #[test]
    fn encode_map() {
        let mut map = BTreeMap::new();
        map.insert("key1".to_string(), TypedArg::Symbol("val1".to_string()));
        map.insert("key2".to_string(), TypedArg::U32("42".to_string()));
        let arg = TypedArg::Map(map);
        let encoded = arg.encode();
        assert!(encoded.contains(r#""key1":"val1""#) || encoded.contains(r#""key1":"val1"#));
        assert!(encoded.contains(r#""key2":"42""#) || encoded.contains(r#""key2":"42"#));
    }

    #[test]
    fn encode_nested_vec() {
        let arg = TypedArg::Vec(vec![
            TypedArg::Vec(vec![
                TypedArg::U32("1".to_string()),
                TypedArg::U32("2".to_string()),
            ]),
            TypedArg::Vec(vec![
                TypedArg::U32("3".to_string()),
                TypedArg::U32("4".to_string()),
            ]),
        ]);
        assert_eq!(arg.encode(), r#"[["1","2"],["3","4"]]"#);
    }

    // ── Serde with integer literals ──────────────────────────────────────

    #[test]
    fn deserialize_u32_as_integer() {
        let toml_str = r#"
args = [{ u32 = 42 }]
"#;
        #[derive(Deserialize)]
        struct TestConfig {
            args: ArgSpec,
        }
        let config: TestConfig = toml::from_str(toml_str).unwrap();
        let encoded = config.args.encode();
        assert_eq!(encoded, vec!["42"]);
    }

    #[test]
    fn deserialize_bool_as_literal() {
        let toml_str = r#"
args = [{ bool = true }, { bool = false }]
"#;
        #[derive(Deserialize)]
        struct TestConfig {
            args: ArgSpec,
        }
        let config: TestConfig = toml::from_str(toml_str).unwrap();
        let encoded = config.args.encode();
        assert_eq!(encoded, vec!["true", "false"]);
    }

    // ── Error cases ─────────────────────────────────────────────────────

    #[test]
    fn deserialize_invalid_type_key_fails() {
        let toml_str = r#"
args = [{ invalid_type = "value" }]
"#;
        #[derive(Deserialize)]
        struct TestConfig {
            args: ArgSpec,
        }
        let result: Result<TestConfig, _> = toml::from_str(toml_str);
        assert!(result.is_err(), "Expected error for invalid type key");
    }

    #[test]
    fn deserialize_mixed_format_array_fails() {
        // TOML doesn't allow mixing string and table in the same array.
        let toml_str = r#"
args = ["hello", { address = "world" }]
"#;
        #[derive(Deserialize)]
        struct TestConfig {
            args: ArgSpec,
        }
        let result: Result<TestConfig, _> = toml::from_str(toml_str);
        assert!(result.is_err(), "TOML should reject mixed-type arrays");
    }

    // ── Legacy backward compatibility ────────────────────────────────────

    #[test]
    fn encode_typed_simple_types_standalone() {
        let args = vec![
            TypedArg::Address(
                "GCXQH4JQN6J3JHXJ6QZ5W4KQ5X6Y7Z8A9B0C1D2E3F4G5H6I7J8K9L0M".to_string(),
            ),
            TypedArg::Bool(true),
            TypedArg::I128("100".to_string()),
            TypedArg::I128("0".to_string()),
        ];
        let encoded: Vec<String> = args.iter().map(|a| a.encode()).collect();
        assert_eq!(encoded.len(), 4);
        assert_eq!(
            encoded[0],
            "GCXQH4JQN6J3JHXJ6QZ5W4KQ5X6Y7Z8A9B0C1D2E3F4G5H6I7J8K9L0M"
        );
        assert_eq!(encoded[1], "true");
        assert_eq!(encoded[2], "100");
        assert_eq!(encoded[3], "0");
    }
}
