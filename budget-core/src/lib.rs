use std::collections::HashMap;
use std::path::Path;

#[derive(Clone, Debug)]
pub enum BudgetLimit {
    Int(u64),
    EnvVar(String),
    Config(String),
}

#[derive(Clone, Debug)]
pub struct BudgetLimitWithPct {
    pub limit: BudgetLimit,
    pub pct: Option<u64>,
}

#[derive(Clone, Debug, Default)]
pub struct BudgetSpec {
    pub cpu: Option<BudgetLimit>,
    pub mem: Option<BudgetLimit>,
    pub pct: Option<u64>,
    pub env_ident: Option<String>,
}

#[derive(Debug)]
pub enum ConfigResolution {
    Value(u64),
    MissingFile,
    MalformedJson,
    KeyNotFound,
}

pub fn resolve_config_value(key: &str) -> ConfigResolution {
    let path = Path::new("budget.json");
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return ConfigResolution::MissingFile,
    };
    let parsed: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return ConfigResolution::MalformedJson,
    };
    match parsed.get(key).and_then(|v| v.as_u64()) {
        Some(n) => ConfigResolution::Value(n),
        None => ConfigResolution::KeyNotFound,
    }
}

pub fn percentage_of(value: u64, pct: u64) -> u64 {
    // Widen before multiplying: `value * pct` overflows `u64` for any value
    // above `u64::MAX / pct`, which panics in debug builds rather than
    // returning a budget figure. Saturate on the way back down so a `pct`
    // above 100 clamps instead of wrapping.
    u64::try_from(u128::from(value) * u128::from(pct) / 100).unwrap_or(u64::MAX)
}

#[derive(serde::Deserialize, Default, Debug)]
pub struct BudgetToml {
    pub network: Option<String>,
    pub source: Option<String>,
    #[serde(default)]
    pub functions: HashMap<String, FunctionConfig>,
}

#[derive(serde::Deserialize, Default, Debug)]
#[serde(deny_unknown_fields)]
pub struct FunctionConfig {
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub cpu_limit: Option<u64>,
    #[serde(default)]
    pub read_limit: Option<u64>,
    #[serde(default)]
    pub write_limit: Option<u64>,
}

pub fn limit_for_metric(func_config: &FunctionConfig, metric: &str) -> Option<u64> {
    match metric {
        "CPU Instructions" => func_config.cpu_limit,
        "Read Bytes" => func_config.read_limit,
        "Write Bytes" => func_config.write_limit,
        _ => None,
    }
}

pub fn evaluate_check(value: u32, limit: Option<u64>) -> (Option<u64>, Option<bool>) {
    match limit {
        Some(limit_value) => (Some(limit_value), Some(u64::from(value) <= limit_value)),
        None => (None, None),
    }
}

pub fn emit_check_failure_entries(
    reports: &mut Vec<CostReport>,
    package_name: &str,
    function: &str,
    func_config: &FunctionConfig,
) {
    for metric in ["CPU Instructions", "Read Bytes", "Write Bytes"] {
        let limit = limit_for_metric(func_config, metric);
        reports.push(CostReport {
            package: package_name.to_string(),
            function: function.to_string(),
            metric,
            value: None,
            limit,
            pass: Some(false),
        });
    }
}

#[derive(serde::Serialize)]
pub struct CostReport {
    pub package: String,
    pub function: String,
    pub metric: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pass: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentage_of_basic() {
        assert_eq!(percentage_of(1000, 110), 1100);
    }

    #[test]
    fn percentage_of_zero() {
        assert_eq!(percentage_of(0, 110), 0);
    }

    #[test]
    fn percentage_of_no_pct() {
        assert_eq!(percentage_of(1000, 100), 1000);
    }

    #[test]
    fn evaluate_check_value_within_limit() {
        let (limit, pass) = evaluate_check(500, Some(1000));
        assert_eq!(limit, Some(1000));
        assert_eq!(pass, Some(true));
    }

    #[test]
    fn evaluate_check_value_exceeds_limit() {
        let (limit, pass) = evaluate_check(1500, Some(1000));
        assert_eq!(limit, Some(1000));
        assert_eq!(pass, Some(false));
    }

    #[test]
    fn evaluate_check_no_limit() {
        let (limit, pass) = evaluate_check(500, None);
        assert_eq!(limit, None);
        assert_eq!(pass, None);
    }

    #[test]
    fn limit_for_metric_cpu() {
        let config = FunctionConfig {
            args: vec![],
            cpu_limit: Some(5_000_000),
            read_limit: None,
            write_limit: None,
        };
        assert_eq!(
            limit_for_metric(&config, "CPU Instructions"),
            Some(5_000_000)
        );
    }

    #[test]
    fn limit_for_metric_read_bytes() {
        let config = FunctionConfig {
            args: vec![],
            cpu_limit: None,
            read_limit: Some(1_000),
            write_limit: None,
        };
        assert_eq!(limit_for_metric(&config, "Read Bytes"), Some(1_000));
    }

    #[test]
    fn limit_for_metric_write_bytes() {
        let config = FunctionConfig {
            args: vec![],
            cpu_limit: None,
            read_limit: None,
            write_limit: Some(500),
        };
        assert_eq!(limit_for_metric(&config, "Write Bytes"), Some(500));
    }

    #[test]
    fn limit_for_metric_unknown_metric() {
        let config = FunctionConfig::default();
        assert_eq!(limit_for_metric(&config, "WASM Bytes"), None);
    }

    #[test]
    fn cost_report_serialization() {
        let report = CostReport {
            package: "test-pkg".to_string(),
            function: "do_work".to_string(),
            metric: "CPU Instructions",
            value: Some(1_000_000),
            limit: Some(5_000_000),
            pass: Some(true),
        };
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("test-pkg"));
        assert!(json.contains("do_work"));
        assert!(json.contains("CPU Instructions"));
    }

    // Property-based tests using proptest
    mod proptests {
        use super::*;
        use proptest::prelude::*;

        // Configure proptest with a fixed seed for deterministic CI runs
        fn config() -> ProptestConfig {
            ProptestConfig {
                rng_algorithm: proptest::test_runner::RngAlgorithm::ChaCha,
                cases: 1000,
                ..ProptestConfig::default()
            }
        }

        // Property tests for percentage_of function
        proptest! {
            #![proptest_config(config())]

            /// Property: percentage_of should never panic
            #[test]
            fn percentage_of_no_panic(value in any::<u64>(), pct in any::<u64>()) {
                let _ = percentage_of(value, pct);
            }

            /// Property: percentage_of should not wraparound
            /// BUG FOUND: This function can overflow when value * pct exceeds u64::MAX.
            /// For example: percentage_of(u64::MAX, 2) will wraparound due to unchecked multiplication.
            /// The function uses `value * pct / 100` which can overflow before the division occurs.
            #[test]
            fn percentage_of_no_wraparound(value in any::<u64>(), pct in any::<u64>()) {
                let result = percentage_of(value, pct);

                // Check for overflow: if pct <= 100, result should not be less than value
                // (assuming no wraparound). If pct > 100, result should be >= value.
                if pct <= 100 && value > 0 {
                    prop_assert!(result <= value,
                        "Expected result <= value when pct <= 100, but got {} > {} (pct={})",
                        result, value, pct);
                }

                // For pct > 100, we can check using checked operations
                if let Some(product) = value.checked_mul(pct) {
                    let expected = product / 100;
                    prop_assert_eq!(result, expected,
                        "Result doesn't match expected when no overflow should occur");
                }
            }

            /// Property: percentage_of(0, _) should always return 0
            #[test]
            fn percentage_of_zero_value(pct in any::<u64>()) {
                prop_assert_eq!(percentage_of(0, pct), 0);
            }

            /// Property: percentage_of(_, 100) should return the original value
            #[test]
            fn percentage_of_hundred_percent(value in any::<u64>()) {
                prop_assert_eq!(percentage_of(value, 100), value);
            }

            /// Property: percentage_of(_, 0) should always return 0
            #[test]
            fn percentage_of_zero_percent(value in any::<u64>()) {
                prop_assert_eq!(percentage_of(value, 0), 0);
            }
        }

        // Property tests for evaluate_check function
        proptest! {
            #![proptest_config(config())]

            /// Property: evaluate_check with None limit should return (None, None)
            #[test]
            fn evaluate_check_none_limit(value in any::<u32>()) {
                let (limit, pass) = evaluate_check(value, None);
                prop_assert_eq!(limit, None);
                prop_assert_eq!(pass, None);
            }

            /// Property: evaluate_check should always return the same limit that was passed in
            #[test]
            fn evaluate_check_returns_limit(value in any::<u32>(), limit in any::<u64>()) {
                let (returned_limit, _) = evaluate_check(value, Some(limit));
                prop_assert_eq!(returned_limit, Some(limit));
            }

            /// Property: evaluate_check with value < limit should pass
            #[test]
            fn evaluate_check_value_below_limit(
                value in 0u32..u32::MAX,
                offset in 1u64..=1000
            ) {
                let limit = u64::from(value).saturating_add(offset);
                let (returned_limit, pass) = evaluate_check(value, Some(limit));
                prop_assert_eq!(returned_limit, Some(limit));
                prop_assert_eq!(pass, Some(true));
            }

            /// Property: evaluate_check with value == limit should pass
            #[test]
            fn evaluate_check_value_equals_limit(value in any::<u32>()) {
                let limit = u64::from(value);
                let (returned_limit, pass) = evaluate_check(value, Some(limit));
                prop_assert_eq!(returned_limit, Some(limit));
                prop_assert_eq!(pass, Some(true));
            }

            /// Property: evaluate_check with value > limit should fail
            #[test]
            fn evaluate_check_value_above_limit(
                limit in 0u64..u64::from(u32::MAX),
                offset in 1u32..=1000
            ) {
                let value = (limit as u32).saturating_add(offset);
                if u64::from(value) > limit {
                    let (returned_limit, pass) = evaluate_check(value, Some(limit));
                    prop_assert_eq!(returned_limit, Some(limit));
                    prop_assert_eq!(pass, Some(false));
                }
            }

            /// Property: evaluate_check should never panic
            #[test]
            fn evaluate_check_no_panic(value in any::<u32>(), limit in any::<Option<u64>>()) {
                let _ = evaluate_check(value, limit);
            }
        }

        // Property tests for resolve_config_value function
        proptest! {
            #![proptest_config(config())]

            /// Property: resolve_config_value should never panic
            #[test]
            fn resolve_config_value_no_panic(key in "\\PC{1,50}") {
                let _ = resolve_config_value(&key);
            }

            /// Property: resolve_config_value should always return one of the valid variants
            #[test]
            fn resolve_config_value_valid_variant(key in "\\PC{1,50}") {
                let result = resolve_config_value(&key);
                match result {
                    ConfigResolution::Value(_) => {},
                    ConfigResolution::MissingFile => {},
                    ConfigResolution::MalformedJson => {},
                    ConfigResolution::KeyNotFound => {},
                }
            }
        }

        // Property tests for limit_for_metric function
        proptest! {
            #![proptest_config(config())]

            /// Property: limit_for_metric should return Some for known metrics when the corresponding field is set
            #[test]
            fn limit_for_metric_cpu_instructions(
                cpu_limit in any::<Option<u64>>(),
                read_limit in any::<Option<u64>>(),
                write_limit in any::<Option<u64>>()
            ) {
                let config = FunctionConfig {
                    args: vec![],
                    cpu_limit,
                    read_limit,
                    write_limit,
                };
                let result = limit_for_metric(&config, "CPU Instructions");
                prop_assert_eq!(result, cpu_limit);
            }

            /// Property: limit_for_metric should return Some for Read Bytes when read_limit is set
            #[test]
            fn limit_for_metric_read_bytes(
                cpu_limit in any::<Option<u64>>(),
                read_limit in any::<Option<u64>>(),
                write_limit in any::<Option<u64>>()
            ) {
                let config = FunctionConfig {
                    args: vec![],
                    cpu_limit,
                    read_limit,
                    write_limit,
                };
                let result = limit_for_metric(&config, "Read Bytes");
                prop_assert_eq!(result, read_limit);
            }

            /// Property: limit_for_metric should return Some for Write Bytes when write_limit is set
            #[test]
            fn limit_for_metric_write_bytes(
                cpu_limit in any::<Option<u64>>(),
                read_limit in any::<Option<u64>>(),
                write_limit in any::<Option<u64>>()
            ) {
                let config = FunctionConfig {
                    args: vec![],
                    cpu_limit,
                    read_limit,
                    write_limit,
                };
                let result = limit_for_metric(&config, "Write Bytes");
                prop_assert_eq!(result, write_limit);
            }

            /// Property: limit_for_metric should return None for unknown metrics
            #[test]
            fn limit_for_metric_unknown(
                cpu_limit in any::<Option<u64>>(),
                read_limit in any::<Option<u64>>(),
                write_limit in any::<Option<u64>>(),
                unknown_metric in "[A-Z][a-z]+ [A-Z][a-z]+"
            ) {
                prop_assume!(unknown_metric != "CPU Instructions");
                prop_assume!(unknown_metric != "Read Bytes");
                prop_assume!(unknown_metric != "Write Bytes");

                let config = FunctionConfig {
                    args: vec![],
                    cpu_limit,
                    read_limit,
                    write_limit,
                };
                let result = limit_for_metric(&config, &unknown_metric);
                prop_assert_eq!(result, None);
            }

            /// Property: limit_for_metric should work with partially populated configs
            #[test]
            fn limit_for_metric_partial_config(
                cpu_limit in any::<Option<u64>>()
            ) {
                let config = FunctionConfig {
                    args: vec![],
                    cpu_limit,
                    read_limit: None,
                    write_limit: None,
                };

                // CPU Instructions should return cpu_limit
                prop_assert_eq!(limit_for_metric(&config, "CPU Instructions"), cpu_limit);
                // Other known metrics should return None
                prop_assert_eq!(limit_for_metric(&config, "Read Bytes"), None);
                prop_assert_eq!(limit_for_metric(&config, "Write Bytes"), None);
            }

            /// Property: limit_for_metric should never panic
            #[test]
            fn limit_for_metric_no_panic(
                cpu_limit in any::<Option<u64>>(),
                read_limit in any::<Option<u64>>(),
                write_limit in any::<Option<u64>>(),
                metric in "\\PC{1,50}"
            ) {
                let config = FunctionConfig {
                    args: vec![],
                    cpu_limit,
                    read_limit,
                    write_limit,
                };
                let _ = limit_for_metric(&config, &metric);
            }

            /// Property: limit_for_metric with default config should return None for all metrics
            #[test]
            fn limit_for_metric_default_config(metric in "\\PC{1,50}") {
                let config = FunctionConfig::default();
                let result = limit_for_metric(&config, &metric);
                prop_assert_eq!(result, None);
            }
        }
    }
}
