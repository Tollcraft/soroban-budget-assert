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

/// Parse a TOML string into a [`Config`], filling missing fields from
/// [`Config::default`].
///
/// Parsing is deliberately lenient:
/// - An empty string or whitespace-only input returns the default config.
/// - A comment-only file returns the default config.
/// - Unknown TOML keys are silently ignored (handled by `serde`).
/// - A malformed TOML string returns the default config rather than
///   propagating a parse error — the tool must always produce output, even
///   when the config file is temporarily broken.
pub fn parse_config(content: &str) -> Config {
    let base = Config::default();

    // Attempt to deserialise into PartialConfig, which has every field
    // Optional so that missing keys fall back to defaults below.
    let partial = match toml::from_str::<PartialConfig>(content) {
        Ok(p) => p,
        // Any parse failure (invalid syntax, type mismatch, etc.) is treated
        // as "no config provided" so the caller always gets a usable Config.
        Err(_) => return base,
    };

    apply_partial(base, partial)
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

    #[test]
    fn malformed_toml_returns_defaults() {
        let content = "network = \n"; // invalid TOML
        let config = parse_config(content);
        assert_eq!(config, Config::default());
    }

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
