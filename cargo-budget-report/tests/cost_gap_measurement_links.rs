//! Cross-reference test for the measurement-status table in `docs/src/cost_gap.md`.
//!
//! The table tells contract authors which operations have a network-verified
//! local-vs-network gap, which only have a local figure, and which have nothing
//! recorded yet. Getting that wrong in either direction costs them: calling a
//! measured operation "unmeasured" makes them over-provision margins, and
//! quoting a figure that is not in the evidence makes them trust a number that
//! was never captured. This test keeps every row tied to `MEASUREMENTS.md` (or a
//! capture fixture) so the table cannot drift from the data.

use std::fs;
use std::path::PathBuf;

/// Prefix of every evidence link in the guide (the guide is published by GitBook,
/// so it links to the repository on GitHub rather than by relative path).
const REPO_BLOB_URL: &str = "https://github.com/Tollcraft/soroban-budget-assert/blob/main/";

const STATUS_HEADING: &str = "## Measurement Status by Operation";

/// Operations the table must classify. The last six were previously all listed
/// as unmeasured, whatever data `MEASUREMENTS.md` held for them.
const REQUIRED_OPERATIONS: &[&str] = &[
    "Storage writes",
    "VM arithmetic",
    "Host-function calls",
    "Map operations",
    "Authorization checks",
    "TTL extensions",
    "Event emissions",
    "Cross-contract invocations",
    "Memory bytes",
    "Storage reads",
];

struct StatusRow {
    operation: String,
    status: String,
    figure: String,
    evidence: String,
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cargo-budget-report has a parent directory")
        .to_path_buf()
}

fn read(relative: &str) -> String {
    fs::read_to_string(repo_root().join(relative))
        .unwrap_or_else(|err| panic!("read {relative}: {err}"))
}

/// Data rows of the first table under [`STATUS_HEADING`], as four trimmed cells.
fn status_rows(guide: &str) -> Vec<StatusRow> {
    let section = guide
        .split(STATUS_HEADING)
        .nth(1)
        .expect("docs/src/cost_gap.md must contain the measurement status section");
    section
        .lines()
        .skip_while(|line| !line.trim_start().starts_with('|'))
        .take_while(|line| line.trim_start().starts_with('|'))
        .skip(2) // header row and `|---|` separator
        .map(|line| {
            let cells: Vec<String> = line
                .trim()
                .trim_matches('|')
                .split('|')
                .map(|cell| cell.trim().replace("**", ""))
                .collect();
            let [operation, status, figure, evidence]: [String; 4] =
                cells.try_into().unwrap_or_else(|cells: Vec<String>| {
                    panic!(
                        "status row must have 4 columns, got {}: {line}",
                        cells.len()
                    )
                });
            StatusRow {
                operation,
                status,
                figure,
                evidence,
            }
        })
        .collect()
}

/// URLs of every Markdown link (`[text](url)`) in a table cell.
fn links(cell: &str) -> Vec<&str> {
    cell.split("](")
        .skip(1)
        .filter_map(|rest| rest.split_once(')').map(|(url, _)| url))
        .collect()
}

/// GitHub's heading anchor: lowercase, punctuation other than `-` and `_`
/// dropped, spaces turned into `-`.
fn github_slug(heading: &str) -> String {
    heading
        .trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_'))
        .map(|c| if c == ' ' { '-' } else { c })
        .collect()
}

/// `(anchor, level, line index)` for each heading outside code fences. Repeated
/// headings get GitHub's `-1`, `-2`, ... suffixes.
fn headings(lines: &[&str]) -> Vec<(String, usize, usize)> {
    let mut seen: Vec<String> = Vec::new();
    let mut in_fence = false;
    let mut out = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let level = line.chars().take_while(|c| *c == '#').count();
        if level == 0 || !line[level..].starts_with(' ') {
            continue;
        }
        let base = github_slug(&line[level..]);
        let repeats = seen.iter().filter(|slug| **slug == base).count();
        seen.push(base.clone());
        let anchor = if repeats == 0 {
            base
        } else {
            format!("{base}-{repeats}")
        };
        out.push((anchor, level, index));
    }
    out
}

/// Text of the section an anchor points at: its heading down to the next
/// heading of the same or a higher level.
fn section(lines: &[&str], anchor: &str) -> Option<String> {
    let all = headings(lines);
    let position = all.iter().position(|(slug, _, _)| slug == anchor)?;
    let (level, start) = (all[position].1, all[position].2);
    let end = all[position + 1..]
        .iter()
        .find(|(_, other, _)| *other <= level)
        .map_or(lines.len(), |(_, _, index)| *index);
    Some(lines[start..end].join("\n"))
}

/// Makes `2,864,886`, `−17.2%` and `-17.2` comparable across Markdown and JSON.
fn normalize(text: &str) -> String {
    text.replace('−', "-").replace([',', '+', '%'], "")
}

/// Every number quoted in a figure cell, normalized.
fn quoted_figures(cell: &str) -> Vec<String> {
    cell.split(|c: char| c.is_whitespace() || matches!(c, '(' | ')' | '/'))
        .filter(|word| word.chars().any(|c| c.is_ascii_digit()))
        .map(normalize)
        .collect()
}

/// The text a row's evidence links point at, concatenated.
fn evidence_text(row: &StatusRow, measurement_lines: &[&str]) -> String {
    let label = &row.operation;
    let targets = links(&row.evidence);
    assert!(
        !targets.is_empty(),
        "{label}: the Evidence column must link to a MEASUREMENTS.md section or a capture fixture"
    );
    let mut text = String::new();
    for target in targets {
        let relative = target.strip_prefix(REPO_BLOB_URL).unwrap_or_else(|| {
            panic!("{label}: evidence must link into the repository ({REPO_BLOB_URL}...), got {target}")
        });
        let (path, anchor) = relative.split_once('#').unwrap_or((relative, ""));
        if path == "MEASUREMENTS.md" {
            assert!(
                !anchor.is_empty(),
                "{label}: link to a specific MEASUREMENTS.md section, not the whole file"
            );
            let body = section(measurement_lines, anchor).unwrap_or_else(|| {
                panic!("{label}: MEASUREMENTS.md has no heading with anchor #{anchor}")
            });
            text.push_str(&body);
        } else {
            text.push_str(&read(path));
        }
        text.push('\n');
    }
    text
}

#[test]
fn cost_gap_classifies_every_previously_unmeasured_operation() {
    let rows = status_rows(&read("docs/src/cost_gap.md"));
    for operation in REQUIRED_OPERATIONS {
        assert!(
            rows.iter().any(|row| row.operation.starts_with(operation)),
            "the cost_gap.md status table has no row for {operation:?}"
        );
    }
}

#[test]
fn cost_gap_status_rows_match_their_measurement_evidence() {
    let guide = read("docs/src/cost_gap.md");
    let measurements = read("MEASUREMENTS.md");
    let measurement_lines: Vec<&str> = measurements.lines().collect();
    let rows = status_rows(&guide);
    assert!(!rows.is_empty(), "the cost_gap.md status table has no rows");

    for row in &rows {
        let label = &row.operation;
        let evidence = evidence_text(row, &measurement_lines);
        let normalized_evidence = normalize(&evidence);
        let figures = quoted_figures(&row.figure);

        match row.status.as_str() {
            "Network-verified" | "Local only" => {
                assert!(
                    !figures.is_empty(),
                    "{label}: a {} row must quote the figure it relies on",
                    row.status
                );
                for figure in &figures {
                    assert!(
                        normalized_evidence.contains(figure.as_str()),
                        "{label}: figure {figure} does not appear in the linked evidence"
                    );
                }
                if row.status == "Local only" {
                    assert!(
                        evidence.contains("| — |") || evidence.to_lowercase().contains("pending"),
                        "{label}: the linked evidence no longer shows the network figure as \
                         pending; if it has landed, mark the row Network-verified and quote its delta"
                    );
                }
            }
            "Pending" => assert!(
                figures.is_empty(),
                "{label}: a Pending row must not quote a figure, found {figures:?}"
            ),
            other => panic!(
                "{label}: unknown status {other:?}; use Network-verified, Local only or Pending"
            ),
        }
    }
}

#[test]
fn github_slug_matches_github_anchor_rules() {
    assert_eq!(
        github_slug(" Authorization (require_auth) measurement"),
        "authorization-require_auth-measurement"
    );
    assert_eq!(
        github_slug(" Cross-contract call depth"),
        "cross-contract-call-depth"
    );
    assert_eq!(github_slug(" TTL extension"), "ttl-extension");
}

#[test]
fn repeated_headings_get_numbered_anchors_and_fenced_lines_are_ignored() {
    let doc = [
        "## Figures",
        "```bash",
        "# not a heading",
        "```",
        "### Figures",
        "## Next",
    ];
    let anchors: Vec<String> = headings(&doc).into_iter().map(|(a, _, _)| a).collect();
    assert_eq!(anchors, ["figures", "figures-1", "next"]);
    assert_eq!(
        section(&doc, "figures").as_deref(),
        Some(doc[..5].join("\n").as_str())
    );
}

#[test]
fn quoted_figures_normalize_markdown_and_json_forms_alike() {
    assert_eq!(quoted_figures("−6.9% to −19.8%"), ["-6.9", "-19.8"]);
    assert_eq!(
        quoted_figures("444,536 CPU (instance) / 458,090 CPU (persistent)"),
        ["444536", "458090"]
    );
    assert!(quoted_figures("—").is_empty());
    assert_eq!(
        normalize("\"delta_percent\": -10.7"),
        "\"delta_percent\": -10.7"
    );
}
