use crate::BudgetToml;
use anyhow::{Context, Result};
use std::collections::HashSet;
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
        // Scoped so the mutable borrow of `child.stdin` ends here: the handle
        // is released before `wait_with_output` takes the child by value.
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

// ─────────────────────────────────────────────────────────────────────
// `budget.toml` schema validation (issue #399)
//
// `load_budget_toml` deserializes into a permissive `BudgetToml` (the
// top-level struct does *not* use `deny_unknown_fields`) so that an unknown
// top-level key is silently dropped. That silence is the damaging failure mode
// the issue targets: a misspelled function name yields a report that simply
// omits the function, with no indication anything was wrong. These helpers
// validate the raw document against the schema the tool understands and report
// *every* problem found — with a closest-match suggestion for typos — so a
// misconfigured file takes one round trip to fix rather than five.
// ─────────────────────────────────────────────────────────────────────

/// One problem found while validating `budget.toml`.
#[derive(Debug, PartialEq)]
pub(crate) struct ValidationError {
    pub location: String,
    pub message: String,
}

impl ValidationError {
    fn new(location: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            location: location.into(),
            message: message.into(),
        }
    }
}

/// Top-level keys the schema understands.
const KNOWN_TOP_LEVEL_KEYS: &[&str] = &[
    "network",
    "source",
    "tolerance",
    "margin",
    "scenarios",
    "functions",
    "retry",
];

/// Validate `content` (the raw `budget.toml` text) against the schema the tool
/// understands, using `available_functions` (every function exported by the
/// workspace) to confirm that each configured `[functions.<name>]` exists.
///
/// Returns the parsed [`BudgetToml`] on success, or *every* validation problem
/// found (never just the first).
pub(crate) fn validate_budget_toml(
    content: &str,
    available_functions: &[String],
) -> std::result::Result<BudgetToml, Vec<ValidationError>> {
    let mut errors: Vec<ValidationError> = Vec::new();

    let value: toml::Value = match toml::from_str(content) {
        Ok(v) => v,
        Err(e) => {
            return Err(vec![ValidationError::new(
                "budget.toml",
                format!("could not parse TOML: {e}"),
            )]);
        }
    };

    if let toml::Value::Table(top) = &value {
        // Unknown top-level keys. We only *reject* a key when it is a plausible
        // typo of a known key (so we can suggest the correction); an arbitrary
        // foreign section — for example `[lints]`, which is consumed by the
        // sibling `soroban-cost-linter` tool — is silently accepted so a single
        // shared `budget.toml` can serve multiple tools without errors.
        for key in top.keys() {
            if !KNOWN_TOP_LEVEL_KEYS.contains(&key.as_str()) {
                if let Some(s) = closest_match(key, KNOWN_TOP_LEVEL_KEYS) {
                    errors.push(ValidationError::new(
                        "budget.toml",
                        format!("unknown top-level key `{key}` (did you mean `{s}`?)"),
                    ));
                }
            }
        }

        // Function existence: every key under `[functions.*]` must be an
        // exported function of the workspace. Skipped when we have no exported
        // functions to compare against (e.g. nothing was built).
        if !available_functions.is_empty() {
            if let Some(toml::Value::Table(fns)) = top.get("functions") {
                // Membership is asked once per configured function, so a set
                // replaces the previous scan of the whole export list for
                // every name. The typo candidates and the rendered
                // "available functions" hint are built lazily, on the first
                // name that is actually missing, so a config where every
                // function resolves allocates neither.
                let known: HashSet<&str> = available_functions.iter().map(String::as_str).collect();
                let mut hint: Option<(Vec<&str>, String)> = None;
                for name in fns.keys() {
                    if known.contains(name.as_str()) {
                        continue;
                    }
                    let (candidate_list, avail_list) = hint.get_or_insert_with(|| {
                        (
                            available_functions.iter().map(String::as_str).collect(),
                            available_functions.join(", "),
                        )
                    });
                    let suggestion = closest_match(name, candidate_list.as_slice());
                    errors.push(ValidationError::new(
                        "budget.toml [functions]",
                        match suggestion {
                            Some(s) => format!(
                                "function `{name}` is configured in budget.toml but does not exist in the workspace (did you mean `{s}`?). Available functions: {}",
                                avail_list.as_str()
                            ),
                            None => format!(
                                "function `{name}` is configured in budget.toml but does not exist in the workspace. Available functions: {}",
                                avail_list.as_str()
                            ),
                        },
                    ));
                }
            }
        }
    }

    // Type errors and nested-unknown-key errors (FunctionConfig uses
    // `deny_unknown_fields`, so a misspelled limit key is caught here) are
    // surfaced by deserializing into the real `BudgetToml`. The top-level
    // struct intentionally does not deny unknown keys, so those are not
    // double-counted here.
    //
    // The text is deserialized exactly once: the successful value is handed
    // back below instead of being discarded and parsed a second time, which is
    // what the previous `if errors.is_empty() { toml::from_str(content) }` tail
    // did on every valid file.
    match toml::from_str::<BudgetToml>(content) {
        Ok(config) if errors.is_empty() => Ok(config),
        Ok(_) => Err(errors),
        Err(e) => {
            let msg = e.to_string();
            let already_covered = KNOWN_TOP_LEVEL_KEYS.iter().any(|k| msg.contains(k));
            if !already_covered {
                errors.push(ValidationError::new(
                    "budget.toml",
                    format!("schema error: {msg}"),
                ));
            } else if errors.is_empty() {
                // The message already names a known key, so it was not
                // re-reported as a separate schema error, and nothing else
                // rejected the document. Surface the raw deserialization
                // failure anyway: a file the schema check does not cover must
                // not pass silently.
                errors.push(ValidationError::new("budget.toml", msg));
            }
            Err(errors)
        }
    }
}

/// Levenshtein edit distance between two strings, computed into a
/// caller-owned scratch buffer.
///
/// Only the previous row of the recurrence is retained (a single `Vec<usize>`
/// of `b`'s length), instead of the full `(n + 1) × (m + 1)` matrix the
/// previous implementation allocated — which matters because this runs once
/// per candidate for every name in `budget.toml` that fails to resolve.
/// Characters are pulled straight out of the `&str`s as they are visited, so
/// the two `Vec<char>` copies per call are gone as well.
///
/// `scratch` is taken by reference rather than allocated per call so a caller
/// that measures one name against many candidates pays for the row once; the
/// buffer is cleared and resized as needed, so it is safe to reuse for any
/// pair of inputs.
fn edit_distance_into(a: &str, b: &str, scratch: &mut Vec<usize>) -> usize {
    let m = b.chars().count();
    scratch.clear();
    scratch.resize(m + 1, 0);
    for (j, cell) in scratch.iter_mut().enumerate() {
        // Row 0 of the recurrence: the cost of deleting every character of `b`.
        *cell = j;
    }

    for (i, a_char) in a.chars().enumerate() {
        // `prev_diag` carries `dp[i - 1][j - 1]` into the next inner step; the
        // cell it is read from still holds the previous row at this point.
        let mut prev_diag = scratch[0];
        scratch[0] = i + 1;
        for (j, b_char) in b.chars().enumerate() {
            let above = scratch[j + 1];
            let cost = usize::from(a_char != b_char);
            scratch[j + 1] = (above + 1).min(scratch[j] + 1).min(prev_diag + cost);
            prev_diag = above;
        }
    }

    scratch[m]
}

/// Return the closest `option` to `candidate` when the edit distance is small
/// enough to be a plausible typo, else `None`.
///
/// One DP row is reused across every candidate (see [`edit_distance_into`]),
/// and only the winning option is copied into a `String`, once, at the end —
/// the previous version allocated a `String` for every candidate it examined,
/// including the ones it immediately discarded.
fn closest_match(candidate: &str, options: &[&str]) -> Option<String> {
    let candidate_len = candidate.chars().count();
    let threshold = if candidate_len <= 4 { 1 } else { 2 };

    let mut scratch: Vec<usize> = Vec::new();
    let mut best: Option<(usize, &str)> = None;
    for opt in options {
        // Edit distance is at least the difference in lengths, so an option
        // that is already further away than `threshold` characters can never
        // satisfy the final check. Skipping it here keeps the `O(n × m)` DP
        // off the common "nothing is close" path; candidates above the
        // threshold were rejected by the final check anyway, so the result is
        // unchanged.
        if opt.chars().count().abs_diff(candidate_len) > threshold {
            continue;
        }
        let d = edit_distance_into(candidate, opt, &mut scratch);
        match best {
            Some((best_d, _)) if d >= best_d => {}
            _ => best = Some((d, opt)),
        }
    }

    match best {
        Some((d, s)) if d > 0 && d <= threshold && d < candidate_len => Some(s.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod schema_tests {
    use super::*;

    fn avail() -> Vec<String> {
        vec![
            "do_expensive_work".to_string(),
            "extend_instance_ttl".to_string(),
            "require_auth_only".to_string(),
        ]
    }

    #[test]
    fn valid_config_passes() {
        let toml = r#"
tolerance = 0.1

[margin]
cpu_margin = 1.1
memory_margin = 1.1
read_margin = 1.1
write_margin = 1.1

[functions.do_expensive_work]
cpu_limit = 5_000_000
"#;
        assert!(
            validate_budget_toml(toml, &avail()).is_ok(),
            "expected Ok for a valid config"
        );
    }

    #[test]
    fn unknown_top_level_key_gets_suggestion() {
        let errs = validate_budget_toml("tolernce = 0.1\n", &avail()).unwrap_err();
        assert_eq!(errs.len(), 1);
        assert!(
            errs[0].message.contains("unknown top-level key `tolernce`"),
            "got: {}",
            errs[0].message
        );
        assert!(
            errs[0].message.contains("did you mean `tolerance`?"),
            "got: {}",
            errs[0].message
        );
    }

    #[test]
    fn foreign_section_accepted() {
        // `[lints]` is a foreign section for soroban-cost-linter; it must not be
        // flagged as an unknown key, so a shared budget.toml stays valid.
        let toml = "[lints]\ncomplexity = \"warn\"\n";
        assert!(
            validate_budget_toml(toml, &avail()).is_ok(),
            "foreign sections must be silently accepted"
        );
    }

    #[test]
    fn typo_without_close_match_is_accepted() {
        // A key that is not close to any known key is treated as a foreign
        // section and accepted, not rejected.
        assert!(validate_budget_toml("zzzz = 1\n", &avail()).is_ok());
    }

    #[test]
    fn configured_function_not_in_workspace_is_reported() {
        let toml = "[functions.do_expensive_wrk]\ncpu_limit = 5_000_000\n";
        let errs = validate_budget_toml(toml, &avail()).unwrap_err();
        assert!(
            errs.iter().any(|e| e.message.contains("`do_expensive_wrk`")
                && e.message.contains("does not exist in the workspace")
                && e.message.contains("did you mean `do_expensive_work`?")),
            "errors: {:?}",
            errs
        );
    }

    #[test]
    fn missing_function_lists_available() {
        let toml = "[functions.nonexistent]\ncpu_limit = 1\n";
        let errs = validate_budget_toml(toml, &avail()).unwrap_err();
        let msg = &errs[0].message;
        assert!(msg.contains("nonexistent"));
        assert!(msg.contains("do_expensive_work"));
        assert!(msg.contains("extend_instance_ttl"));
        assert!(msg.contains("require_auth_only"));
    }

    #[test]
    fn nested_typo_in_function_reports_schema_error() {
        // `cpu_lmit` is a misspelling of `cpu_limit`; FunctionConfig denies
        // unknown fields, so this surfaces as a schema error naming the field.
        let toml = "[functions.do_expensive_work]\ncpu_lmit = 5_000_000\n";
        let errs = validate_budget_toml(toml, &avail()).unwrap_err();
        assert!(
            errs.iter().any(|e| e.message.contains("cpu_lmit")),
            "errors: {:?}",
            errs
        );
    }

    #[test]
    fn type_error_names_field_and_expected_type() {
        let toml = "[functions.do_expensive_work]\ncpu_limit = \"high\"\n";
        let errs = validate_budget_toml(toml, &avail()).unwrap_err();
        assert!(
            errs.iter().any(
                |e| e.message.contains("cpu_limit") && e.message.to_lowercase().contains("u64")
            ),
            "errors: {:?}",
            errs
        );
    }

    #[test]
    fn all_problems_reported_together() {
        let toml = "tolernce = 0.1\n\n[functions.nonexistent]\ncpu_limit = 1\n";
        let errs = validate_budget_toml(toml, &avail()).unwrap_err();
        assert!(
            errs.len() >= 2,
            "expected at least two errors (unknown key + missing function), got {:?}",
            errs
        );
    }

    #[test]
    fn empty_available_functions_skips_function_check() {
        // When nothing was built, we must not flag configured functions as
        // missing (that would be a false positive).
        let toml = "[functions.do_expensive_work]\ncpu_limit = 1\n";
        assert!(
            validate_budget_toml(toml, &[]).is_ok(),
            "function check must be skipped when no functions are available"
        );
    }

    #[test]
    fn closest_match_threshold() {
        assert_eq!(closest_match("network", &["source", "tolerance"]), None);
        assert_eq!(
            closest_match("tolernce", &["tolerance"]),
            Some("tolerance".to_string())
        );
        assert_eq!(
            closest_match("cpu_lmit", &["cpu_limit"]),
            Some("cpu_limit".to_string())
        );
    }

    // ── edit-distance / typo-suggestion edge cases ────────────────────

    /// Single-shot wrapper over the shared-scratch DP so the assertions below
    /// read like the old helper; production code reuses one buffer across
    /// every candidate it measures.
    fn edit_distance(a: &str, b: &str) -> usize {
        let mut scratch = Vec::new();
        edit_distance_into(a, b, &mut scratch)
    }

    #[test]
    fn edit_distance_matches_known_values() {
        assert_eq!(edit_distance("", ""), 0);
        assert_eq!(edit_distance("", "abc"), 3);
        assert_eq!(edit_distance("abc", ""), 3);
        assert_eq!(edit_distance("kitten", "sitting"), 3);
        assert_eq!(edit_distance("cpu_limit", "cpu_limit"), 0);
        assert_eq!(edit_distance("cpu_lmit", "cpu_limit"), 1);
    }

    #[test]
    fn edit_distance_is_symmetric_over_multibyte_chars() {
        assert_eq!(
            edit_distance("tolernce", "tolerance"),
            edit_distance("tolerance", "tolernce")
        );
        // Characters are compared, not bytes: `é` is one substitution away
        // from `e` even though it encodes as two bytes.
        assert_eq!(edit_distance("café", "cafe"), 1);
        assert_eq!(edit_distance("日本", "日本語"), 1);
    }

    #[test]
    fn edit_distance_reuses_a_single_dp_row() {
        // One row of the recurrence, never the full (n + 1) × (m + 1) table:
        // this is what keeps the typo scan allocation-light.
        let mut scratch = Vec::new();
        assert_eq!(edit_distance_into("aaaaaaaa", "bbbbbbbb", &mut scratch), 8);
        assert_eq!(scratch.len(), 9, "one row, sized by the second operand");
        // A buffer left over from a longer comparison still answers a shorter
        // one correctly, in both directions.
        assert_eq!(edit_distance_into("ab", "ab", &mut scratch), 0);
        assert_eq!(scratch.len(), 3);
        assert_eq!(edit_distance_into("abc", "ab", &mut scratch), 1);
        assert_eq!(scratch.len(), 3);
    }

    #[test]
    fn closest_match_skips_candidates_of_impossible_length() {
        // Edit distance is at least the length difference, so a candidate
        // whose length already exceeds the threshold can never win.
        assert_eq!(
            closest_match("cpu_limit", &["a_much_longer_function_name"]),
            None
        );
        assert_eq!(closest_match("", &["anything"]), None);
    }

    #[test]
    fn closest_match_prefers_the_first_equally_distant_candidate() {
        // Both options are one edit away; the first one examined wins, which
        // is what the previous implementation did too.
        assert_eq!(closest_match("aa", &["ab", "ba"]), Some("ab".to_string()));
    }

    #[test]
    fn closest_match_ignores_an_exact_name() {
        // Distance zero is not a typo, so an exact match is never suggested
        // back to the user.
        assert_eq!(closest_match("tolerance", &["tolerance"]), None);
    }

    #[test]
    fn closest_match_picks_the_nearest_of_many_options() {
        let options = ["tolerance", "network", "source", "margin", "scenarios"];
        assert_eq!(closest_match("margn", &options), Some("margin".to_string()));
        assert_eq!(
            closest_match("scenerios", &options),
            Some("scenarios".to_string())
        );
        // Nothing is within the threshold of `src`, so no suggestion (and no
        // distance computation) is produced for the long options.
        assert_eq!(closest_match("src", &options), None);
    }

    #[test]
    fn closest_match_is_case_sensitive() {
        assert_eq!(
            closest_match("Tolerance", &["tolerance"]),
            Some("tolerance".to_string())
        );
    }

    // ── validate_budget_toml success path and error combination ───────

    #[test]
    fn valid_config_returns_the_parsed_values() {
        // The successful deserialization is handed back to the caller rather
        // than discarded and parsed a second time, so the values it carries
        // must be the ones from the file.
        let toml = "tolerance = 0.25\n\n[functions.do_expensive_work]\ncpu_limit = 5_000_000\n";
        let parsed = validate_budget_toml(toml, &avail()).expect("expected a valid config");
        assert_eq!(parsed.tolerance, Some(0.25));
        assert!(parsed.functions.contains_key("do_expensive_work"));
    }

    #[test]
    fn every_configured_function_present_produces_no_errors() {
        let toml = "[functions.do_expensive_work]\ncpu_limit = 1\n\n\
                    [functions.require_auth_only]\ncpu_limit = 1\n";
        assert!(
            validate_budget_toml(toml, &avail()).is_ok(),
            "no name is missing, so no typo hint is built and no error raised"
        );
    }

    #[test]
    fn each_missing_function_reports_the_available_list() {
        let toml = "[functions.depsoit]\ncpu_limit = 1\n\n[functions.wthdraw]\ncpu_limit = 1\n";
        let errs = validate_budget_toml(toml, &avail()).unwrap_err();
        assert_eq!(errs.len(), 2, "errors: {errs:?}");
        for err in &errs {
            assert_eq!(err.location, "budget.toml [functions]");
            assert!(
                err.message
                    .contains("Available functions: do_expensive_work"),
                "got: {}",
                err.message
            );
        }
    }

    #[test]
    fn top_level_type_error_is_still_reported() {
        // `network` is a known key, so the deserialization failure is not
        // re-reported as an extra schema error; it must still reach the
        // caller rather than letting the document pass.
        let errs = validate_budget_toml("network = 1\n", &avail()).unwrap_err();
        assert_eq!(errs.len(), 1, "errors: {errs:?}");
        assert!(
            errs[0].message.contains("network"),
            "got: {}",
            errs[0].message
        );
    }

    #[test]
    fn unknown_key_is_reported_even_when_the_schema_check_also_fails() {
        let toml = "tolernce = 0.1\n\n[functions.do_expensive_work]\ncpu_limit = \"high\"\n";
        let errs = validate_budget_toml(toml, &avail()).unwrap_err();
        assert!(
            errs.iter().any(|e| e.message.contains("tolernce")),
            "errors: {errs:?}"
        );
    }

    #[test]
    fn unparseable_toml_reports_a_single_parse_error() {
        let errs = validate_budget_toml("this is not toml = =\n", &avail()).unwrap_err();
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].location, "budget.toml");
        assert!(errs[0].message.contains("could not parse TOML"));
    }
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
