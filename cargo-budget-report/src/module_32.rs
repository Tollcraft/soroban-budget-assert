//! Shared test helpers for the cargo-budget-report crate.
//!
//! Consolidates duplicated validation and serialization helpers used
//! across multiple test modules (`tests` and `module_8`).

use crate::CostReport;

/// Serializes a slice of [`CostReport`] to CSV using the same logic as the
/// `--csv` output path in production code.
///
/// * `check = true` — six-column header (`package, function, metric, value,
///   limit, pass`); all reports are included.
/// * `check = false` — four-column header (`package, function, metric,
///   value`); only reports with `value.is_some()` are emitted.
pub(crate) fn reports_to_csv(reports: &[CostReport], check: bool) -> String {
    let mut csv_writer = csv::Writer::from_writer(vec![]);
    if check {
        csv_writer
            .write_record(["package", "function", "metric", "value", "limit", "pass"])
            .unwrap();
        for report in reports {
            let value_str = report
                .value
                .map(|metric_value| metric_value.to_string())
                .unwrap_or_default();
            let limit_str = report
                .limit
                .map(|configured_limit| configured_limit.to_string())
                .unwrap_or_default();
            let pass_str = report
                .pass
                .map(|passed| passed.to_string())
                .unwrap_or_default();
            csv_writer
                .write_record([
                    report.package.as_str(),
                    report.function.as_str(),
                    report.metric,
                    value_str.as_str(),
                    limit_str.as_str(),
                    pass_str.as_str(),
                ])
                .unwrap();
        }
    } else {
        csv_writer
            .write_record(["package", "function", "metric", "value"])
            .unwrap();
        for report in reports {
            if report.value.is_some() {
                let value_str = report
                    .value
                    .map(|metric_value| metric_value.to_string())
                    .unwrap_or_default();
                csv_writer
                    .write_record([
                        report.package.as_str(),
                        report.function.as_str(),
                        report.metric,
                        value_str.as_str(),
                    ])
                    .unwrap();
            }
        }
    }
    csv_writer.flush().unwrap();
    String::from_utf8(csv_writer.into_inner().unwrap()).unwrap()
}

/// Creates a temporary directory, changes the process CWD into it, and
/// returns both the temp-dir guard and the previous CWD so the caller
/// can restore it with [`restore_cwd`].
///
/// Used by `scaffold_init` tests to avoid overwriting the real
/// `budget.toml`.
pub(crate) fn isolate_temp_dir() -> (tempfile::TempDir, std::path::PathBuf) {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let prev = std::env::current_dir().expect("failed to read current working directory");
    std::env::set_current_dir(tmp.path()).expect("failed to change into temp dir");
    (tmp, prev)
}

/// Restores the original CWD after a call to [`isolate_temp_dir`].
pub(crate) fn restore_cwd(prev: &std::path::Path) {
    std::env::set_current_dir(prev).expect("failed to restore original working directory");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;

    #[test]
    fn csv_output_without_check_has_four_columns() {
        let reports = vec![
            CostReport {
                package: "my-contract".to_string(),
                function: "do_work".to_string(),
                metric: "CPU Instructions",
                value: Some(1_000_000),
                limit: None,
                pass: None,
            },
            CostReport {
                package: "my-contract".to_string(),
                function: "do_work".to_string(),
                metric: "Read Bytes",
                value: Some(2_048),
                limit: None,
                pass: None,
            },
        ];
        let csv = reports_to_csv(&reports, false);
        let expected = concat!(
            "package,function,metric,value\n",
            "my-contract,do_work,CPU Instructions,1000000\n",
            "my-contract,do_work,Read Bytes,2048\n",
        );
        assert_eq!(csv, expected);
    }

    #[test]
    fn csv_output_with_check_has_six_columns() {
        let reports = vec![
            CostReport {
                package: "my-contract".to_string(),
                function: "do_work".to_string(),
                metric: "CPU Instructions",
                value: Some(1_000_000),
                limit: Some(5_000_000),
                pass: Some(true),
            },
            CostReport {
                package: "my-contract".to_string(),
                function: "do_work".to_string(),
                metric: "Write Bytes",
                value: Some(4_096),
                limit: Some(1_000),
                pass: Some(false),
            },
        ];
        let csv = reports_to_csv(&reports, true);
        let expected = concat!(
            "package,function,metric,value,limit,pass\n",
            "my-contract,do_work,CPU Instructions,1000000,5000000,true\n",
            "my-contract,do_work,Write Bytes,4096,1000,false\n",
        );
        assert_eq!(csv, expected);
    }

    #[test]
    fn csv_output_without_check_excludes_null_values() {
        let reports = vec![
            CostReport {
                package: "my-contract".to_string(),
                function: "do_work".to_string(),
                metric: "CPU Instructions",
                value: None,
                limit: None,
                pass: None,
            },
            CostReport {
                package: "my-contract".to_string(),
                function: "do_work".to_string(),
                metric: "Read Bytes",
                value: Some(2_048),
                limit: None,
                pass: None,
            },
        ];
        let csv = reports_to_csv(&reports, false);
        let expected = concat!(
            "package,function,metric,value\n",
            "my-contract,do_work,Read Bytes,2048\n",
        );
        assert_eq!(csv, expected);
    }

    #[test]
    fn csv_output_with_check_includes_simulation_failures() {
        let reports = vec![CostReport {
            package: "my-contract".to_string(),
            function: "do_work".to_string(),
            metric: "CPU Instructions",
            value: None,
            limit: Some(5_000_000),
            pass: Some(false),
        }];
        let csv = reports_to_csv(&reports, true);
        let expected = concat!(
            "package,function,metric,value,limit,pass\n",
            "my-contract,do_work,CPU Instructions,,5000000,false\n",
        );
        assert_eq!(csv, expected);
    }

    #[test]
    fn csv_output_empty_reports_produces_header_only() {
        let reports: Vec<CostReport> = vec![];
        let csv = reports_to_csv(&reports, false);
        assert_eq!(csv, "package,function,metric,value\n");
    }

    #[test]
    fn csv_output_check_empty_reports_produces_header_only() {
        let reports: Vec<CostReport> = vec![];
        let csv = reports_to_csv(&reports, true);
        assert_eq!(csv, "package,function,metric,value,limit,pass\n");
    }
}
