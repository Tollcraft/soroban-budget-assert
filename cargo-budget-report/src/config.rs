#![allow(dead_code)]

//! Runtime configuration for `cargo-budget-report`.
//!
//! [`Config`] holds the settings that govern how the tool connects to the
//! Soroban network and retries transient failures.  Values are loaded from a
//! TOML file (typically `budget.toml` at the workspace root) via
//! [`parse_config`].  Any field omitted from the file falls back to the
//! [`Default`] implementation, so a minimal config only needs to specify the
//! fields that differ from the defaults.

/// Runtime settings for a `cargo budget-report` run.
#[derive(Debug, PartialEq)]
pub struct Config {
    /// Soroban network to deploy and measure against.
    ///
    /// Typical values: `"testnet"`, `"futurenet"`, `"local"`.
    /// Defaults to `"testnet"`.
    pub network: String,

    /// Stellar identity / account name used as the deploying source.
    ///
    /// Must be a key name known to the local `stellar` CLI key store.
    /// Defaults to `"default"`.
    pub source: String,

    /// Maximum wall-clock seconds to wait for a single RPC call before
    /// treating it as a timeout error.
    ///
    /// Defaults to `30`.
    pub timeout_secs: u64,

    /// Number of additional attempts after the first failure before giving
    /// up on a deploy or invoke call.
    ///
    /// A value of `3` means up to four total attempts (one initial + three
    /// retries).  Defaults to `3`.
    pub retry_attempts: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            network: "testnet".to_string(),
            source: "default".to_string(),
            timeout_secs: 30,
            retry_attempts: 3,
        }
    }
}

#[derive(Debug)]
pub enum ConfigError {
    Toml(toml::de::Error),
    FieldType {
        field: &'static str,
        message: String,
    },
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Toml(e) => write!(f, "TOML syntax error: {e}"),
            Self::FieldType { field, message } => {
                write!(f, "field '{field}' has invalid type: {message}")
            }
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Toml(e) => Some(e),
            Self::FieldType { .. } => None,
        }
    }
}

impl From<toml::de::Error> for ConfigError {
    fn from(err: toml::de::Error) -> Self {
        Self::Toml(err)
    }
}

/// Fallible configuration parser that reports specific syntax and type errors.
pub fn parse_config_fallible(content: &str) -> Result<Config, ConfigError> {
    let mut config = Config::default();
    let table: toml::Table = toml::from_str(content)?;

    if let Some(val) = table.get("network") {
        config.network = val
            .clone()
            .try_into::<String>()
            .map_err(|e| ConfigError::FieldType {
                field: "network",
                message: e.to_string(),
            })?;
    }
    if let Some(val) = table.get("source") {
        config.source = val
            .clone()
            .try_into::<String>()
            .map_err(|e| ConfigError::FieldType {
                field: "source",
                message: e.to_string(),
            })?;
    }
    if let Some(val) = table.get("timeout_secs") {
        config.timeout_secs =
            val.clone()
                .try_into::<u64>()
                .map_err(|e| ConfigError::FieldType {
                    field: "timeout_secs",
                    message: e.to_string(),
                })?;
    }
    if let Some(val) = table.get("retry_attempts") {
        config.retry_attempts =
            val.clone()
                .try_into::<u32>()
                .map_err(|e| ConfigError::FieldType {
                    field: "retry_attempts",
                    message: e.to_string(),
                })?;
    }

    Ok(config)
}

/// Parse a TOML string into a [`Config`], filling missing fields from
/// [`Config::default`].
///
/// Parsing is deliberately lenient:
/// - An empty string or whitespace-only input returns the default config.
/// - A comment-only file returns the default config.
/// - Unknown TOML keys are silently ignored (handled by `serde`).
/// - When individual fields fail type deserialization, valid sibling fields
///   are preserved while malformed fields fall back to their defaults.
/// - A malformed TOML syntax error returns the default config rather than
///   propagating a parse error — the tool must always produce output, even
///   when the config file is temporarily broken.
pub fn parse_config(content: &str) -> Config {
    let mut config = Config::default();

    let table = match toml::from_str::<toml::Table>(content) {
        Ok(t) => t,
        Err(_) => return config,
    };

    if let Some(val) = table.get("network") {
        if let Ok(net) = val.clone().try_into::<String>() {
            config.network = net;
        }
    }
    if let Some(val) = table.get("source") {
        if let Ok(src) = val.clone().try_into::<String>() {
            config.source = src;
        }
    }
    if let Some(val) = table.get("timeout_secs") {
        if let Ok(t) = val.clone().try_into::<u64>() {
            config.timeout_secs = t;
        }
    }
    if let Some(val) = table.get("retry_attempts") {
        if let Ok(r) = val.clone().try_into::<u32>() {
            config.retry_attempts = r;
        }
    }

    config
}

/// Merge `partial` into `base`, taking each field from `partial` when
/// present and keeping the `base` value otherwise.
///
/// Extracted as a separate function to keep [`parse_config`] focused on
/// I/O concerns (reading and error-handling) while this function handles
/// the pure data-merging logic.  This split also makes unit-testing the
/// merge behaviour independent of TOML parsing.
fn apply_partial(base: Config, partial: PartialConfig) -> Config {
    Config {
        network: partial.network.unwrap_or(base.network),
        source: partial.source.unwrap_or(base.source),
        timeout_secs: partial.timeout_secs.unwrap_or(base.timeout_secs),
        retry_attempts: partial.retry_attempts.unwrap_or(base.retry_attempts),
    }
}

/// Intermediate deserialization target for [`parse_config`].
///
/// Every field is `Option<T>` so that serde leaves missing TOML keys as
/// `None` rather than failing.  [`apply_partial`] then collapses each
/// `None` to the corresponding default.
#[derive(serde::Deserialize)]
struct PartialConfig {
    network: Option<String>,
    source: Option<String>,
    timeout_secs: Option<u64>,
    retry_attempts: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_config_uses_defaults() {
        let config = parse_config("");
        assert_eq!(config, Config::default());
    }

    #[test]
    fn whitespace_config_uses_defaults() {
        let config = parse_config("   \n  \n  ");
        assert_eq!(config, Config::default());
    }

    #[test]
    fn comment_only_config_uses_defaults() {
        let config = parse_config("# just a comment\n# another comment");
        assert_eq!(config, Config::default());
    }

    #[test]
    fn partial_config_fills_missing_from_defaults() {
        let content = r#"network = "futurenet""#;
        let config = parse_config(content);
        assert_eq!(config.network, "futurenet");
        assert_eq!(config.source, Config::default().source);
        assert_eq!(config.timeout_secs, Config::default().timeout_secs);
        assert_eq!(config.retry_attempts, Config::default().retry_attempts);
    }

    #[test]
    fn full_config_overrides_all_defaults() {
        let content = r#"
network = "local"
source = "alice"
timeout_secs = 60
retry_attempts = 5
"#;
        let config = parse_config(content);
        assert_eq!(config.network, "local");
        assert_eq!(config.source, "alice");
        assert_eq!(config.timeout_secs, 60);
        assert_eq!(config.retry_attempts, 5);
    }

    #[test]
    fn malformed_toml_falls_back_to_defaults() {
        let content = "this is not valid toml = = =";
        let config = parse_config(content);
        assert_eq!(config, Config::default());
    }

    #[test]
    fn unknown_keys_are_ignored() {
        let content = r#"
network = "testnet"
unknown_field = "should not cause errors"
"#;
        let config = parse_config(content);
        assert_eq!(config.network, "testnet");
        assert_eq!(config.source, Config::default().source);
    }

    /// Verify apply_partial directly: every Some field overrides the base,
    /// every None keeps the base value.
    #[test]
    fn apply_partial_only_overrides_present_fields() {
        let base = Config::default();
        let partial = PartialConfig {
            network: Some("local".to_string()),
            source: None,
            timeout_secs: None,
            retry_attempts: Some(1),
        };
        let result = apply_partial(base, partial);
        assert_eq!(result.network, "local");
        assert_eq!(result.source, Config::default().source);
        assert_eq!(result.timeout_secs, Config::default().timeout_secs);
        assert_eq!(result.retry_attempts, 1);
    }
}
