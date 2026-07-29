//! Unit tests targeting empty configuration and fallback default edge cases.
//!
//! These tests specifically exercise the configuration parser's behaviour when
//! `budget.toml` is missing, empty, or contains only comments/whitespace,
//! verifying that [`BudgetToml::default()`] supplies sensible fallback values
//! for `network` and `source`.

#[cfg(test)]
mod empty_config_and_defaults_tests {
    use crate::*;

    // ── default_network / default_source unit tests ─────────────────────

    #[test]
    fn default_network_returns_testnet() {
        assert_eq!(default_network(), Some("testnet".to_string()));
    }

    #[test]
    fn default_source_returns_alice() {
        assert_eq!(default_source(), Some("alice".to_string()));
    }

    // ── BudgetToml::default() tests ────────────────────────────────────

    #[test]
    fn budget_toml_default_has_testnet_network() {
        let config = BudgetToml::default();
        assert_eq!(config.network, Some("testnet".to_string()));
    }

    #[test]
    fn budget_toml_default_has_alice_source() {
        let config = BudgetToml::default();
        assert_eq!(config.source, Some("alice".to_string()));
    }

    #[test]
    fn budget_toml_default_has_no_tolerance() {
        let config = BudgetToml::default();
        assert!(config.tolerance.is_none());
    }

    #[test]
    fn budget_toml_default_has_empty_functions() {
        let config = BudgetToml::default();
        assert!(config.functions.is_empty());
    }

    // ── load_budget_toml: missing / empty / whitespace / comments ──────

    #[test]
    fn load_budget_toml_nonexistent_file_returns_defaults_with_network_and_source() {
        let tmp = tempfile::NamedTempFile::new().expect("failed to create temp file");
        // Delete the file so it does not exist.
        let path = tmp.path().to_path_buf();
        std::fs::remove_file(&path).ok();

        let config =
            load_budget_toml(&path).expect("non-existent file should return default BudgetToml");
        assert_eq!(
            config.network,
            Some("testnet".to_string()),
            "default network should be testnet"
        );
        assert_eq!(
            config.source,
            Some("alice".to_string()),
            "default source should be alice"
        );
        assert!(config.tolerance.is_none());
        assert!(config.functions.is_empty());
    }

    #[test]
    fn load_budget_toml_empty_file_returns_defaults_with_network_and_source() {
        let tmp = tempfile::NamedTempFile::new().expect("failed to create temp file");
        std::fs::write(tmp.path(), "").unwrap();

        let config =
            load_budget_toml(tmp.path()).expect("empty file should return default BudgetToml");
        assert_eq!(config.network, Some("testnet".to_string()));
        assert_eq!(config.source, Some("alice".to_string()));
        assert!(config.tolerance.is_none());
        assert!(config.functions.is_empty());
    }

    #[test]
    fn load_budget_toml_whitespace_only_file_returns_defaults() {
        let tmp = tempfile::NamedTempFile::new().expect("failed to create temp file");
        std::fs::write(tmp.path(), "   \n\t\n  \n").unwrap();

        let config =
            load_budget_toml(tmp.path()).expect("whitespace-only file should return default BudgetToml");
        assert_eq!(config.network, Some("testnet".to_string()));
        assert_eq!(config.source, Some("alice".to_string()));
    }

    #[test]
    fn load_budget_toml_comments_only_file_returns_defaults() {
        let tmp = tempfile::NamedTempFile::new().expect("failed to create temp file");
        std::fs::write(tmp.path(), "# This is a comment\n# Another comment\n").unwrap();

        let config =
            load_budget_toml(tmp.path()).expect("comments-only file should return default BudgetToml");
        assert_eq!(config.network, Some("testnet".to_string()));
        assert_eq!(config.source, Some("alice".to_string()));
    }

    #[test]
    fn load_budget_toml_hash_only_comment_line_returns_defaults() {
        let tmp = tempfile::NamedTempFile::new().expect("failed to create temp file");
        std::fs::write(tmp.path(), "#\n").unwrap();

        let config =
            load_budget_toml(tmp.path()).expect("hash-only comment should return default BudgetToml");
        assert_eq!(config.network, Some("testnet".to_string()));
        assert_eq!(config.source, Some("alice".to_string()));
    }

    #[test]
    fn load_budget_toml_newline_only_file_returns_defaults() {
        let tmp = tempfile::NamedTempFile::new().expect("failed to create temp file");
        std::fs::write(tmp.path(), "\n").unwrap();

        let config =
            load_budget_toml(tmp.path()).expect("newline-only file should return default BudgetToml");
        assert_eq!(config.network, Some("testnet".to_string()));
        assert_eq!(config.source, Some("alice".to_string()));
    }

    // ── load_budget_toml: partial configs ──────────────────────────────

    #[test]
    fn load_budget_toml_network_only_uses_default_source() {
        let tmp = tempfile::NamedTempFile::new().expect("failed to create temp file");
        std::fs::write(tmp.path(), "network = \"futurenet\"\n").unwrap();

        let config =
            load_budget_toml(tmp.path()).expect("partial config should parse successfully");
        assert_eq!(config.network.as_deref(), Some("futurenet"));
        assert_eq!(
            config.source,
            Some("alice".to_string()),
            "missing source should fall back to default"
        );
        assert!(config.tolerance.is_none());
        assert!(config.functions.is_empty());
    }

    #[test]
    fn load_budget_toml_source_only_uses_default_network() {
        let tmp = tempfile::NamedTempFile::new().expect("failed to create temp file");
        std::fs::write(tmp.path(), "source = \"bob\"\n").unwrap();

        let config =
            load_budget_toml(tmp.path()).expect("partial config should parse successfully");
        assert_eq!(
            config.network,
            Some("testnet".to_string()),
            "missing network should fall back to default"
        );
        assert_eq!(config.source.as_deref(), Some("bob"));
        assert!(config.tolerance.is_none());
        assert!(config.functions.is_empty());
    }

    #[test]
    fn load_budget_toml_explicit_network_and_source_override_defaults() {
        let tmp = tempfile::NamedTempFile::new().expect("failed to create temp file");
        std::fs::write(
            tmp.path(),
            "network = \"local\"\nsource = \"dev\"\n",
        )
        .unwrap();

        let config =
            load_budget_toml(tmp.path()).expect("explicit config should parse successfully");
        assert_eq!(config.network.as_deref(), Some("local"));
        assert_eq!(config.source.as_deref(), Some("dev"));
    }

    // ── load_budget_toml: empty string values ──────────────────────────

    #[test]
    fn load_budget_toml_empty_network_string_overrides_default() {
        let tmp = tempfile::NamedTempFile::new().expect("failed to create temp file");
        std::fs::write(tmp.path(), "network = \"\"\n").unwrap();

        let config =
            load_budget_toml(tmp.path()).expect("empty network string should parse");
        assert_eq!(
            config.network,
            Some(String::new()),
            "explicit empty string should override default"
        );
        assert_eq!(config.source, Some("alice".to_string()));
    }

    #[test]
    fn load_budget_toml_empty_source_string_overrides_default() {
        let tmp = tempfile::NamedTempFile::new().expect("failed to create temp file");
        std::fs::write(tmp.path(), "source = \"\"\n").unwrap();

        let config =
            load_budget_toml(tmp.path()).expect("empty source string should parse");
        assert_eq!(config.network, Some("testnet".to_string()));
        assert_eq!(
            config.source,
            Some(String::new()),
            "explicit empty string should override default"
        );
    }

    // ── load_budget_toml: error cases ──────────────────────────────────

    #[test]
    fn load_budget_toml_invalid_toml_returns_error() {
        let tmp = tempfile::NamedTempFile::new().expect("failed to create temp file");
        std::fs::write(tmp.path(), "[[[invalid toml]]]\n").unwrap();

        let result = load_budget_toml(tmp.path());
        assert!(result.is_err(), "invalid TOML should produce an error");
    }

    #[test]
    fn load_budget_toml_read_error_on_nonexistent_directory() {
        let result = load_budget_toml("/nonexistent_directory/budget.toml");
        assert!(result.is_err(), "non-existent directory should produce an error");
    }

    // ── resolve_tolerance configuration interaction tests ──────────────

    #[test]
    fn resolve_tolerance_uses_config_tolerance_when_no_cli_override() {
        let config = BudgetToml {
            network: Some("testnet".to_string()),
            source: Some("alice".to_string()),
            tolerance: Some(0.15),
            functions: HashMap::new(),
        };
        let tolerance = resolve_tolerance(None, &config).expect("should resolve");
        assert!((tolerance.value - 0.15).abs() < f64::EPSILON);
    }

    #[test]
    fn resolve_tolerance_falls_back_to_default_when_not_configured() {
        let config = BudgetToml::default();
        // BudgetToml::default() has tolerance = None
        let tolerance = resolve_tolerance(None, &config).expect("should resolve to default");
        assert!((tolerance.value - 0.05).abs() < f64::EPSILON);
    }

    // ── load_budget_toml: minimal valid config ─────────────────────────

    #[test]
    fn load_budget_toml_function_with_no_limit_fields_uses_defaults() {
        let tmp = tempfile::NamedTempFile::new().expect("failed to create temp file");
        let content = r#"
network = "testnet"
source = "alice"

[functions.ping]
args = ["--n", "1"]
"#;
        std::fs::write(tmp.path(), content).unwrap();

        let config = load_budget_toml(tmp.path()).expect("minimal function config should parse");
        let func = config.functions.get("ping").expect("ping should be present");
        assert_eq!(func.args.encode(), vec!["--n".to_string(), "1".to_string()]);
        assert!(func.cpu_limit.is_none());
        assert!(func.read_limit.is_none());
        assert!(func.write_limit.is_none());
        assert!(func.tolerance.is_none());
    }
}
