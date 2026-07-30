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
    value * pct / 100
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
}
