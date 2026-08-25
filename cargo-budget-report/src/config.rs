#![allow(dead_code)]

#[derive(Debug, PartialEq)]
pub struct Config {
    pub network: String,
    pub source: String,
    pub timeout_secs: u64,
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

pub fn parse_config(content: &str) -> Config {
    let base = Config::default();

    let parsed: Result<PartialConfig, _> = toml::from_str(content);
    let partial = match parsed {
        Ok(p) => p,
        Err(_) => return base,
    };

    Config {
        network: partial.network.unwrap_or(base.network),
        source: partial.source.unwrap_or(base.source),
        timeout_secs: partial.timeout_secs.unwrap_or(base.timeout_secs),
        retry_attempts: partial.retry_attempts.unwrap_or(base.retry_attempts),
    }
}

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
}
