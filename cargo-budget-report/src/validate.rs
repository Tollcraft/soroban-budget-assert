use anyhow::{Context, Result};
use std::io::Write;
use std::process::{Command, Stdio};

/// Resource metrics decoded by the Stellar CLI's own XDR decoder.
#[derive(Debug, Clone, PartialEq)]
pub struct CliDecodedMetrics {
    pub instructions: u64,
    pub read_bytes: u64,
    pub write_bytes: u64,
}

/// Outcome of validating one function's metrics against the Stellar CLI.
#[derive(Debug)]
pub enum ValidationResult {
    /// Every metric matched exactly.
    Match,
    /// One or more metrics differed.
    Mismatch {
        /// Human-readable diagnostics describing each discrepancy.
        diagnostics: Vec<String>,
    },
    /// Validation could not run (prerequisites missing, CLI unavailable, etc.).
    Skipped {
        /// Reason for skipping.
        reason: String,
    },
}

/// Check whether the Stellar CLI is available for validation.
pub fn cli_is_available() -> bool {
    Command::new("stellar")
        .arg("--version")
        .output()
        .ok()
        .is_some_and(|o| o.status.success())
}

/// Decode a SorobanTransactionData XDR using the Stellar CLI's own xdr decoder
/// and extract the three resource metrics.
pub fn decode_with_cli(xdr_b64: &str) -> Result<CliDecodedMetrics> {
    let mut child = Command::new("stellar")
        .args([
            "xdr",
            "decode",
            "--type",
            "SorobanTransactionData",
            "--output",
            "json",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to spawn stellar xdr decode")?;

    {
        let stdin = child.stdin.as_mut().context("failed to open stdin")?;
        stdin
            .write_all(xdr_b64.as_bytes())
            .context("failed to write XDR to stellar stdin")?;
    }

    let output = child
        .wait_with_output()
        .context("failed to read stellar xdr decode output")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("stellar xdr decode failed: {}", stderr);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_xdr_decode_output(&stdout)
        .with_context(|| format!("failed to parse Stellar CLI xdr decode output: {}", stdout))
}

/// Parse the JSON output of `stellar xdr decode --type SorobanTransactionData --output json`.
///
/// The CLI outputs a JSON representation of the decoded XDR. We expect a
/// structure containing `resources.instructions`, `resources.read_bytes` (or
/// `resources.disk_read_bytes`), and `resources.write_bytes`.
fn parse_xdr_decode_output(output: &str) -> Result<CliDecodedMetrics> {
    use serde_json::Value;

    let root: Value = serde_json::from_str(output)
        .with_context(|| format!("CLI xdr decode output is not valid JSON:\n{}", output))?;

    let resources = root
        .get("resources")
        .context("CLI output missing 'resources' field")?;

    let instructions = resources
        .get("instructions")
        .and_then(|v| v.as_u64())
        .context("CLI output missing or invalid 'resources.instructions'")?;

    let read_bytes = resources
        .get("read_bytes")
        .or_else(|| resources.get("disk_read_bytes"))
        .and_then(|v| v.as_u64())
        .context("CLI output missing 'resources.read_bytes' or 'resources.disk_read_bytes'")?;

    let write_bytes = resources
        .get("write_bytes")
        .and_then(|v| v.as_u64())
        .context("CLI output missing or invalid 'resources.write_bytes'")?;

    Ok(CliDecodedMetrics {
        instructions,
        read_bytes,
        write_bytes,
    })
}

/// Compare cargo-budget-report metrics with CLI-decoded metrics.
///
/// Returns `Match` if every metric agrees exactly, or `Mismatch` with detailed
/// diagnostics for each differing metric. No automatic correction is applied.
pub fn compare_metrics(
    report_instructions: u32,
    report_read_bytes: u32,
    report_write_bytes: u32,
    cli: &CliDecodedMetrics,
) -> ValidationResult {
    let mut mismatches = Vec::new();

    let report_cpu = u64::from(report_instructions);
    if report_cpu != cli.instructions {
        mismatches.push(format!(
            "CPU Instructions: cargo-budget-report = {} (0x{:x}), Stellar CLI = {} (0x{:x})",
            report_cpu, report_cpu, cli.instructions, cli.instructions
        ));
    }

    let report_read = u64::from(report_read_bytes);
    if report_read != cli.read_bytes {
        mismatches.push(format!(
            "Read Bytes: cargo-budget-report = {}, Stellar CLI = {}",
            report_read, cli.read_bytes
        ));
    }

    let report_write = u64::from(report_write_bytes);
    if report_write != cli.write_bytes {
        mismatches.push(format!(
            "Write Bytes: cargo-budget-report = {}, Stellar CLI = {}",
            report_write, cli.write_bytes
        ));
    }

    if mismatches.is_empty() {
        ValidationResult::Match
    } else {
        ValidationResult::Mismatch {
            diagnostics: mismatches,
        }
    }
}

/// Run the full validation for a single function: decode the same XDR through
/// the Stellar CLI and compare every metric.
///
/// Returns `Skipped` if the CLI or its xdr decode subcommand is not available.
pub fn validate_metrics(
    xdr_b64: &str,
    report_instructions: u32,
    report_read_bytes: u32,
    report_write_bytes: u32,
) -> ValidationResult {
    let cli_metrics = match decode_with_cli(xdr_b64) {
        Ok(m) => m,
        Err(e) => {
            return ValidationResult::Skipped {
                reason: format!("Stellar CLI xdr decode failed: {:#}", e),
            };
        }
    };

    compare_metrics(
        report_instructions,
        report_read_bytes,
        report_write_bytes,
        &cli_metrics,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use stellar_xdr::{LedgerFootprint, SorobanTransactionDataExt, VecM};
    use stellar_xdr::{Limits, SorobanTransactionData, WriteXdr};

    const FIXTURE_INSTRUCTIONS: u32 = 1_000_000;
    const FIXTURE_READ_BYTES: u32 = 2_048;
    const FIXTURE_WRITE_BYTES: u32 = 4_096;

    fn make_fixture_tx_data() -> SorobanTransactionData {
        SorobanTransactionData {
            ext: SorobanTransactionDataExt::V0,
            resources: stellar_xdr::SorobanResources {
                footprint: LedgerFootprint {
                    read_only: VecM::default(),
                    read_write: VecM::default(),
                },
                instructions: FIXTURE_INSTRUCTIONS,
                disk_read_bytes: FIXTURE_READ_BYTES,
                write_bytes: FIXTURE_WRITE_BYTES,
            },
            resource_fee: 0,
        }
    }

    fn fixture_xdr_b64() -> String {
        let tx_data = make_fixture_tx_data();
        tx_data
            .to_xdr_base64(Limits::none())
            .expect("failed to encode fixture SorobanTransactionData")
    }

    // ── parse_xdr_decode_output tests ──────────────────────────────────

    #[test]
    fn parses_matching_metrics_from_cli_json() {
        let json = r#"{
            "resources": {
                "instructions": 1000000,
                "read_bytes": 2048,
                "write_bytes": 4096
            }
        }"#;
        let metrics = parse_xdr_decode_output(json).expect("should parse valid JSON");
        assert_eq!(metrics.instructions, 1_000_000);
        assert_eq!(metrics.read_bytes, 2_048);
        assert_eq!(metrics.write_bytes, 4_096);
    }

    #[test]
    fn parses_disk_read_bytes_alias() {
        let json = r#"{
            "resources": {
                "instructions": 500000,
                "disk_read_bytes": 1024,
                "write_bytes": 2048
            }
        }"#;
        let metrics = parse_xdr_decode_output(json).expect("should parse disk_read_bytes alias");
        assert_eq!(metrics.instructions, 500_000);
        assert_eq!(metrics.read_bytes, 1_024);
        assert_eq!(metrics.write_bytes, 2_048);
    }

    #[test]
    fn parse_fails_on_missing_resources() {
        let result = parse_xdr_decode_output("{}");
        assert!(result.is_err());
    }

    #[test]
    fn parse_fails_on_missing_instructions() {
        let json = r#"{"resources": {"read_bytes": 0, "write_bytes": 0}}"#;
        let result = parse_xdr_decode_output(json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_fails_on_null_instructions() {
        let json = r#"{"resources": {"instructions": null, "read_bytes": 0, "write_bytes": 0}}"#;
        let result = parse_xdr_decode_output(json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_fails_on_non_json_output() {
        let result = parse_xdr_decode_output("not json at all");
        assert!(result.is_err());
    }

    // ── compare_metrics tests ──────────────────────────────────────────

    #[test]
    fn compare_metrics_match_returns_match() {
        let cli = CliDecodedMetrics {
            instructions: 1_000_000,
            read_bytes: 2_048,
            write_bytes: 4_096,
        };
        let result = compare_metrics(1_000_000, 2_048, 4_096, &cli);
        assert!(
            matches!(result, ValidationResult::Match),
            "expected Match, got {:?}",
            result
        );
    }

    #[test]
    fn compare_metrics_cpu_mismatch_reports_diagnostic() {
        let cli = CliDecodedMetrics {
            instructions: 999_999,
            read_bytes: 2_048,
            write_bytes: 4_096,
        };
        let result = compare_metrics(1_000_000, 2_048, 4_096, &cli);
        match result {
            ValidationResult::Mismatch { diagnostics } => {
                assert!(!diagnostics.is_empty());
                assert!(diagnostics[0].contains("CPU Instructions"));
                assert!(diagnostics[0].contains("cargo-budget-report"));
                assert!(diagnostics[0].contains("Stellar CLI"));
            }
            other => panic!("expected Mismatch, got {:?}", other),
        }
    }

    #[test]
    fn compare_metrics_read_bytes_mismatch_reports_diagnostic() {
        let cli = CliDecodedMetrics {
            instructions: 1_000_000,
            read_bytes: 2_047,
            write_bytes: 4_096,
        };
        let result = compare_metrics(1_000_000, 2_048, 4_096, &cli);
        match result {
            ValidationResult::Mismatch { diagnostics } => {
                assert!(diagnostics.iter().any(|d| d.contains("Read Bytes")));
            }
            other => panic!("expected Mismatch, got {:?}", other),
        }
    }

    #[test]
    fn compare_metrics_write_bytes_mismatch_reports_diagnostic() {
        let cli = CliDecodedMetrics {
            instructions: 1_000_000,
            read_bytes: 2_048,
            write_bytes: 4_095,
        };
        let result = compare_metrics(1_000_000, 2_048, 4_096, &cli);
        match result {
            ValidationResult::Mismatch { diagnostics } => {
                assert!(diagnostics.iter().any(|d| d.contains("Write Bytes")));
            }
            other => panic!("expected Mismatch, got {:?}", other),
        }
    }

    #[test]
    fn compare_metrics_all_mismatch_reports_three_diagnostics() {
        let cli = CliDecodedMetrics {
            instructions: 0,
            read_bytes: 0,
            write_bytes: 0,
        };
        let result = compare_metrics(1_000_000, 2_048, 4_096, &cli);
        match result {
            ValidationResult::Mismatch { diagnostics } => {
                assert_eq!(diagnostics.len(), 3);
            }
            other => panic!("expected Mismatch, got {:?}", other),
        }
    }

    #[test]
    fn compare_metrics_very_large_values() {
        let cli = CliDecodedMetrics {
            instructions: u64::MAX,
            read_bytes: u64::MAX,
            write_bytes: u64::MAX,
        };
        let result = compare_metrics(u32::MAX, u32::MAX, u32::MAX, &cli);
        assert!(
            matches!(result, ValidationResult::Mismatch { .. }),
            "expected Mismatch for u32::MAX vs u64::MAX, got {:?}",
            result
        );
    }

    // ── validate_metrics tests ────────────────────────────────────────

    #[test]
    fn validate_metrics_round_trip_match_or_skip() {
        let xdr = fixture_xdr_b64();
        let result = validate_metrics(
            &xdr,
            FIXTURE_INSTRUCTIONS,
            FIXTURE_READ_BYTES,
            FIXTURE_WRITE_BYTES,
        );
        match result {
            ValidationResult::Match => {}
            ValidationResult::Skipped { .. } => {}
            ValidationResult::Mismatch { diagnostics } => {
                panic!("unexpected mismatch in round-trip: {:?}", diagnostics);
            }
        }
    }

    #[test]
    fn validate_metrics_reports_mismatch_or_skip() {
        let xdr = fixture_xdr_b64();
        let result = validate_metrics(&xdr, 0, 0, 0);
        match result {
            ValidationResult::Match => {
                panic!("expected mismatch when values differ");
            }
            ValidationResult::Mismatch { diagnostics } => {
                assert!(!diagnostics.is_empty());
                assert!(diagnostics[0].contains("CPU Instructions"));
            }
            ValidationResult::Skipped { .. } => {}
        }
    }
}
