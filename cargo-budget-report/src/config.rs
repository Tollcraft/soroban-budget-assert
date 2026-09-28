//! Configuration parsing and management for budget.toml.
//!
//! This module provides the [`Config`] struct and [`parse_config`] function
//! for reading and merging configuration from TOML files with sensible
//! defaults.
//!
//! # Examples
//!
//! ```rust
//! use config::parse_config;
//!
//! // Empty input yields defaults
//! let config = parse_config("");
//! assert_eq!(config.network, "testnet");
//! assert_eq!(config.timeout_secs, 30);
//!
//! // Partial input fills missing fields from defaults
//! let config = parse_config(r#"network = "futurenet""#);
//! assert_eq!(config.network, "futurenet");
//! ```

use serde::Deserialize;

/// Runtime configuration parameters for the budget reporting tool.
///
/// All fields have sensible defaults that are applied when the
/// corresponding TOML key is missing or the file is empty.
#[derive(Debug, PartialEq)]
pub struct Config {
    /// Target Stellar network (e.g., "testnet", "futurenet", "local").
    pub network: String,
    /// Source account keypair name for Stellar CLI operations.
    pub source: String,
    /// Network request timeout in seconds.
    pub timeout_secs: u64,
    /// Number of retry attempts for failed network operations.
    pub retry_attempts: u32,
}

impl Default for Config {
    /// Returns the default configuration with testnet settings.
    ///
    /// Defaults:
    /// - `network`: `"testnet"`
    /// - `source`: `"default"`
    /// - `timeout_secs`: `30`
    /// - `retry_attempts`: `3`
    fn default() -> Self {
        Self {
            network: "testnet".to_string(),
            source: "default".to_string(),
            timeout_secs: 30,
            retry_attempts: 3,
        }
    }
}

/// Parse a TOML configuration string into a [`Config`].
///
/// If the input is empty, contains only comments, or has invalid TOML,
/// the default configuration is returned. Missing fields are filled
/// from defaults. Unknown fields are silently ignored.
///
/// # Arguments
///
/// * `content` - A TOML-formatted configuration string.
///
/// # Returns
///
/// A [`Config`] with values parsed from `content`, falling back to
/// defaults for any missing fields.
pub fn parse_config(content: &str) -> Config {
    let base = Config::default();

    // Attempt to parse the content as TOML. If parsing fails for any
    // reason (empty string, malformed TOML, only comments), we fall
    // back to the default configuration.
    let parsed: Result<PartialConfig, _> = toml::from_str(content);
    let partial = match parsed {
        Ok(p) => p,
        Err(_) => return base,
    };

    // Merge parsed values with defaults: use the parsed value if present,
    // otherwise fall back to the default. This ensures that missing
    // fields in the TOML file get sensible defaults.
    Config {
        network: partial.network.unwrap_or(base.network),
        source: partial.source.unwrap_or(base.source),
        timeout_secs: partial.timeout_secs.unwrap_or(base.timeout_secs),
        retry_attempts: partial.retry_attempts.unwrap_or(base.retry_attempts),
    }
}

/// A partial configuration used for deserialization.
///
/// All fields are `Option` types so that missing keys in the TOML file
/// can be distinguished from fields with explicit values. The
/// `PartialConfig` is merged with defaults in [`parse_config`].
#[derive(serde::Deserialize)]
struct PartialConfig {
    /// Optional network override.
    network: Option<String>,
    /// Optional source account override.
    source: Option<String>,
    /// Optional timeout override in seconds.
    timeout_secs: Option<u64>,
    /// Optional retry attempts override.
    retry_attempts: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Empty string should return the default configuration.
    #[test]
    fn empty_config_uses_defaults() {
        let config = parse_config("");
        assert_eq!(config, Config::default());
    }

    /// Whitespace-only input should return defaults.
    #[test]
    fn whitespace_config_uses_defaults() {
        let config = parse_config("   \n  \n  ");
        assert_eq!(config, Config::default());
    }

    /// Comment-only TOML should return defaults.
    #[test]
    fn comment_only_config_uses_defaults() {
        let config = parse_config("# just a comment\n# another comment");
        assert_eq!(config, Config::default());
    }

    /// Partial TOML should fill missing fields from defaults.
    #[test]
    fn partial_config_fills_missing_from_defaults() {
        let content = r#"network = "futurenet""#;
        let config = parse_config(content);
        assert_eq!(config.network, "futurenet");
        assert_eq!(config.source, Config::default().source);
        assert_eq!(config.timeout_secs, Config::default().timeout_secs);
        assert_eq!(config.retry_attempts, Config::default().retry_attempts);
    }

    /// Full TOML should override all defaults.
    #[test]
    fn full_config_overrides_all_defaults() {
        let content = r#"
network = "local"
source = "bob"
timeout_secs = 60
retry_attempts = 5
"#;
        let config = parse_config(content);
        assert_eq!(config.network, "local");
        assert_eq!(config.source, "bob");
        assert_eq!(config.timeout_secs, 60);
        assert_eq!(config.retry_attempts, 5);
    }

    /// Malformed TOML should return defaults gracefully.
    #[test]
    fn malformed_toml_returns_defaults() {
        let content = "network = \n"; // invalid TOML
        let config = parse_config(content);
        assert_eq!(config, Config::default());
    }

    /// Unknown fields should be ignored without error.
    #[test]
    fn unknown_fields_ignored_gracefully() {
        let content = r#"
network = "testnet"
unknown_field = "should not cause errors"
"#;
        let config = parse_config(content);
        assert_eq!(config.network, "testnet");
        assert_eq!(config.source, Config::default().source);
    }
}
