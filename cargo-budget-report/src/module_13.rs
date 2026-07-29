//! # Configuration Fallback Defaults
//!
//! This module provides safe fallback logic for missing or empty configuration
//! values throughout the `soroban-budget-assert` tooling.
//!
//! # Problem
//!
//! When `budget.toml` or `budget.json` is empty or missing a required field,
//! the parser can fail ungracefully instead of using safe defaults.
//!
//! # Solution
//!
//! All configuration resolvers in this module return `Option<T>` rather than
//! panicking. Callers can chain `.unwrap_or(default_value)` to pick up safe
//! defaults without crashing.
//!
//! See [`resolve_limit_from_json`] for the JSON-config fallback logic and
//! [`resolve_toml_network`] for the TOML-config fallback logic.

/// Attempts to resolve a budget limit from a `budget.json` file.
///
/// Looks up `key` in the JSON object and returns:
/// * `Some(value)` — the key was found and parsed as a valid `u64`.
/// * `None` — the file is absent, empty, malformed, or the key is missing.
///
/// Callers should treat `None` as "no limit" and fall back to `u64::MAX`.
///
/// # Examples
///
/// ```
/// use cargo_budget_report::module_13::resolve_limit_from_json;
///
/// // File doesn't exist → None (safe default)
/// let result = resolve_limit_from_json("/nonexistent/budget.json", "cpu_limit");
/// assert_eq!(result, None);
/// ```
pub fn resolve_limit_from_json(path: &str, key: &str) -> Option<u64> {
    let content = std::fs::read_to_string(path).ok()?;

    // Defensive: treat an empty file as "no config available"
    if content.trim().is_empty() {
        return None;
    }

    // Parse as JSON, falling back if it's invalid
    let parsed: serde_json::Value = serde_json::from_str(&content).ok()?;

    // Extract the key and try to convert to u64
    parsed.get(key)?.as_u64()
}

/// Resolves the target network from a `BudgetToml` config, falling back to
/// `"testnet"` when no value is configured.
///
/// # Examples
///
/// ```
/// use cargo_budget_report::module_13::resolve_toml_network;
///
/// assert_eq!(resolve_toml_network(None), "testnet");
/// assert_eq!(resolve_toml_network(Some("futurenet".to_string())), "futurenet");
/// ```
pub fn resolve_toml_network(config_value: Option<String>) -> String {
    config_value.unwrap_or_else(|| "testnet".to_string())
}

/// Resolves the source account from a `BudgetToml` config, falling back to
/// `"alice"` when no value is configured.
///
/// # Examples
///
/// ```
/// use cargo_budget_report::module_13::resolve_toml_source;
///
/// assert_eq!(resolve_toml_source(None), "alice");
/// assert_eq!(resolve_toml_source(Some("bob".to_string())), "bob");
/// ```
pub fn resolve_toml_source(config_value: Option<String>) -> String {
    config_value.unwrap_or_else(|| "alice".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_json_path() -> PathBuf {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time is before UNIX_EPOCH")
            .as_nanos();
        path.push(format!("budget_test_{}.json", nanos));
        path
    }

    // ── JSON config tests ────────────────────────────────────────────────

    #[test]
    fn empty_json_file_returns_none() {
        let path = unique_json_path();
        fs::write(&path, "").expect("failed to write empty JSON file");

        let result = resolve_limit_from_json(path.to_str().unwrap(), "cpu_limit");
        let _ = fs::remove_file(&path);

        assert_eq!(result, None, "empty file should return None (safe default)");
    }

    #[test]
    fn missing_key_returns_none() {
        let path = unique_json_path();
        fs::write(&path, r#"{"other_key": 100}"#).expect("failed to write JSON file");

        let result = resolve_limit_from_json(path.to_str().unwrap(), "cpu_limit");
        let _ = fs::remove_file(&path);

        assert_eq!(
            result, None,
            "missing key should return None (safe default)"
        );
    }

    #[test]
    fn malformed_json_returns_none() {
        let path = unique_json_path();
        fs::write(&path, "this is not valid json").expect("failed to write malformed JSON");

        let result = resolve_limit_from_json(path.to_str().unwrap(), "cpu_limit");
        let _ = fs::remove_file(&path);

        assert_eq!(
            result, None,
            "malformed JSON should return None (safe default)"
        );
    }

    #[test]
    fn valid_key_returns_the_value() {
        let path = unique_json_path();
        fs::write(&path, r#"{"cpu_limit": 5000000}"#).expect("failed to write JSON file");

        let result = resolve_limit_from_json(path.to_str().unwrap(), "cpu_limit");
        let _ = fs::remove_file(&path);

        assert_eq!(result, Some(5_000_000));
    }

    #[test]
    fn missing_file_returns_none() {
        let result = resolve_limit_from_json("/nonexistent/path/budget.json", "cpu_limit");
        assert_eq!(result, None);
    }

    #[test]
    fn whitespace_only_file_returns_none() {
        let path = unique_json_path();
        fs::write(&path, "   \n\t  ").expect("failed to write whitespace-only file");

        let result = resolve_limit_from_json(path.to_str().unwrap(), "cpu_limit");
        let _ = fs::remove_file(&path);

        assert_eq!(
            result, None,
            "whitespace-only file should return None (safe default)"
        );
    }

    // ── TOML config tests ─────────────────────────────────────────────────

    #[test]
    fn toml_network_falls_back_to_testnet() {
        assert_eq!(resolve_toml_network(None), "testnet");
    }

    #[test]
    fn toml_network_uses_configured_value() {
        assert_eq!(
            resolve_toml_network(Some("futurenet".to_string())),
            "futurenet"
        );
    }

    #[test]
    fn toml_source_falls_back_to_alice() {
        assert_eq!(resolve_toml_source(None), "alice");
    }

    #[test]
    fn toml_source_uses_configured_value() {
        assert_eq!(resolve_toml_source(Some("bob".to_string())), "bob");
    }
}
