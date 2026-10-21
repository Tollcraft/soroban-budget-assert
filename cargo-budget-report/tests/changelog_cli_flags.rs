//! Cross-check the CLI flags named in `CHANGELOG.md` against the flags the
//! binary actually exposes.
//!
//! Issue #901: the `## Unreleased (prior) > Added` section of the changelog
//! advertised a `--share-threshold` flag, a share percentage fetched live via
//! `getNetworkLimits`, and `resource_limit` / `share_pct` fields in the
//! `--json` output. None of that shipped: `cargo-budget-report/src/cli/args.rs`
//! declares no share-threshold field, and the JSON envelope carries only
//! `schema_version` and `snapshots` rows of `package` / `function` / `metric` /
//! `value` / `limit` / `pass`. A user copying the entry got
//! `error: unexpected argument '--share-threshold' found`.
//!
//! `scripts/check-cli-docs.sh` only reports code-to-doc gaps, so a phantom
//! flag in the changelog was invisible to CI. These tests close the gap in the
//! other direction:
//!
//! 1. every flag the changelog names must exist in `src/cli/args.rs`, and
//! 2. every flag in `src/cli/args.rs` must be listed by the binary's own
//!    `--help` output, and
//! 3. the network-share names that were never shipped must not reappear in the
//!    changelog.

use assert_cmd::Command;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate lives one level below the repo root")
        .to_path_buf()
}

fn changelog_text() -> String {
    let path = repo_root().join("CHANGELOG.md");
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

fn args_source() -> String {
    let path = repo_root().join("cargo-budget-report/src/cli/args.rs");
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// Flags quoted in the changelog that belong to another tool.
///
/// `--features` is cargo's own flag: the `scripts/regenerate-measurements.sh`
/// entry quotes that script's invocation, not a `cargo budget-report` flag.
const FOREIGN_FLAGS: &[&str] = &["--features"];

/// A flag-shaped token found in the changelog.
///
/// `family` is `Some(prefix)` for a wildcard such as `--margin-*`, which
/// stands for "every flag starting with this prefix" rather than for one
/// exact flag name.
#[derive(Debug, PartialEq, Eq)]
struct FlagToken {
    name: String,
    family: Option<String>,
}

/// Extract the flag-shaped tokens from `text`.
///
/// A token is `--` followed by at least one lowercase letter or digit. A
/// trailing `*` (or a `{` opening a brace expansion) marks a family of flags
/// rather than one exact name.
fn flag_tokens(text: &str) -> Vec<FlagToken> {
    let bytes = text.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'-' || i + 1 >= bytes.len() || bytes[i + 1] != b'-' {
            i += 1;
            continue;
        }
        let start = i;
        let mut j = i + 2;
        // A flag name starts with a letter or a digit: a markdown table
        // separator (`|---|`) is a run of dashes, not a flag.
        if j >= bytes.len() || !bytes[j].is_ascii_alphanumeric() {
            i = j;
            continue;
        }
        while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'-') {
            j += 1;
        }
        let name = text[start..j].to_string();
        i = j;
        if name.len() <= 2 {
            continue;
        }
        if j < bytes.len() && (bytes[j] == b'*' || bytes[j] == b'{') {
            tokens.push(FlagToken {
                name: String::new(),
                family: Some(name),
            });
            continue;
        }
        tokens.push(FlagToken { name, family: None });
    }
    tokens
}

/// Every concrete flag named in the changelog, plus the wildcard families.
fn changelog_flags() -> (BTreeSet<String>, BTreeSet<String>) {
    let mut concrete = BTreeSet::new();
    let mut families = BTreeSet::new();
    for token in flag_tokens(&changelog_text()) {
        match token.family {
            Some(prefix) => {
                families.insert(prefix);
            }
            None => {
                if !FOREIGN_FLAGS.contains(&token.name.as_str()) {
                    concrete.insert(token.name);
                }
            }
        }
    }
    (concrete, families)
}

/// The long flags declared in `cargo-budget-report/src/cli/args.rs`.
///
/// Every field carrying an `#[arg(...)]` attribute becomes a `--flag`; clap
/// derives the long name from the field name in kebab-case unless the
/// attribute names one explicitly.
fn flags_from_args_source() -> BTreeSet<String> {
    let source = args_source();
    let mut flags = BTreeSet::new();
    // The `#[arg(...)]` attribute of the field that follows it; `None` when the
    // next field carries no argument attribute.
    let mut pending_attribute: Option<String> = None;
    let mut collecting_attribute = false;

    for line in source.lines() {
        let trimmed = line.trim();
        if collecting_attribute {
            // An attribute may wrap over several lines; it ends at `)]`.
            let Some(attribute) = pending_attribute.as_mut() else {
                continue;
            };
            attribute.push(' ');
            attribute.push_str(trimmed);
            if trimmed.ends_with(")]") {
                collecting_attribute = false;
            }
            continue;
        }
        if trimmed.starts_with("#[arg(") {
            pending_attribute = Some(trimmed.to_string());
            if !trimmed.ends_with(")]") {
                collecting_attribute = true;
            }
            continue;
        }
        if trimmed.starts_with("#[") {
            // Any other attribute ends the `#[arg(...)]` association.
            pending_attribute = None;
            continue;
        }
        let Some(field) = trimmed.strip_prefix("pub ") else {
            continue;
        };
        let Some((name, _)) = field.split_once(':') else {
            continue;
        };
        let Some(attribute) = pending_attribute.take() else {
            continue;
        };
        let long = match explicit_long_name(&attribute) {
            Some(explicit) => explicit,
            None => name.trim().to_string(),
        };
        flags.insert(format!("--{}", long.replace('_', "-")));
    }
    assert!(
        !flags.is_empty(),
        "no flags parsed from cargo-budget-report/src/cli/args.rs - the parser broke"
    );
    flags
}

/// The `long = "..."` override inside an `#[arg(...)]` attribute, if any.
fn explicit_long_name(attribute: &str) -> Option<String> {
    let rest = attribute.split_once("long")?.1;
    let rest = rest.trim_start();
    let rest = rest.strip_prefix('=')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// The long flags the binary advertises in its own `--help` output.
fn flags_from_help() -> BTreeSet<String> {
    let mut cmd = Command::cargo_bin("cargo-budget-report").expect("binary should be built");
    // The binary is a cargo subcommand: the real flags live behind the
    // leading `budget-report` token.
    let help = cmd
        .arg("budget-report")
        .arg("--help")
        .output()
        .expect("failed to run the binary");
    assert!(
        help.status.success(),
        "`cargo budget-report --help` failed: {}",
        String::from_utf8_lossy(&help.stderr)
    );
    parse_help_flags(&String::from_utf8_lossy(&help.stdout))
}

/// Pull the long flags out of a clap `--help` listing.
///
/// Only the flag column is read: a line whose first token starts with `-` is
/// an option definition, and its long form is the `--name` part. Wrapped
/// help prose is indented into the description column and therefore does not
/// start with `-`.
fn parse_help_flags(help: &str) -> BTreeSet<String> {
    let mut flags = BTreeSet::new();
    for line in help.lines() {
        let mut tokens = line.split_whitespace();
        let Some(first) = tokens.next() else {
            continue;
        };
        let long = if first.starts_with("--") {
            Some(first)
        } else {
            // clap prints short and long forms together: `-h, --help`.
            match tokens.next() {
                Some(second) if first.starts_with('-') && second.starts_with("--") => Some(second),
                _ => None,
            }
        };
        let Some(long) = long else {
            continue;
        };
        let name = long.trim_start_matches("--").trim_end_matches(',');
        if name.is_empty() {
            continue;
        }
        flags.insert(format!("--{name}"));
    }
    flags
}

/// Every flag the changelog names must exist in the CLI's argument definition.
#[test]
fn every_flag_named_in_the_changelog_exists_in_the_cli() {
    let (concrete, families) = changelog_flags();
    let declared = flags_from_args_source();
    assert!(
        !concrete.is_empty(),
        "no flags found in CHANGELOG.md - the scanner broke"
    );

    let missing: Vec<&String> = concrete
        .iter()
        .filter(|flag| !declared.contains(*flag))
        .collect();
    assert!(
        missing.is_empty(),
        "flags named in CHANGELOG.md but absent from \
         cargo-budget-report/src/cli/args.rs: {missing:?}"
    );

    let empty_families: Vec<&String> = families
        .iter()
        .filter(|prefix| {
            !declared
                .iter()
                .any(|flag| flag.starts_with(prefix.as_str()))
        })
        .collect();
    assert!(
        empty_families.is_empty(),
        "flag families named in CHANGELOG.md with no matching flag in \
         cargo-budget-report/src/cli/args.rs: {empty_families:?}"
    );
}

/// Every flag the argument definition declares must be listed by the binary.
///
/// This is the "reproducible against the current binary" half: the changelog
/// check above is only as good as the flag list, and the flag list is only
/// good if the shipped binary really has those flags.
#[test]
fn every_declared_flag_is_listed_by_the_binary_help() {
    let declared = flags_from_args_source();
    let advertised = flags_from_help();
    let missing: Vec<&String> = declared
        .iter()
        .filter(|flag| !advertised.contains(*flag))
        .collect();
    assert!(
        missing.is_empty(),
        "flags declared in cargo-budget-report/src/cli/args.rs but absent from \
         `cargo budget-report --help`: {missing:?}"
    );
}

/// The network-share names the changelog used to advertise were never
/// shipped, so they must not come back as if they were.
#[test]
fn the_changelog_does_not_advertise_unimplemented_network_share_names() {
    let text = changelog_text();
    for name in [
        "--share-threshold",
        "share_pct",
        "resource_limit",
        "getNetworkLimits",
    ] {
        assert!(
            !text.contains(name),
            "CHANGELOG.md names `{name}`, which the CLI does not implement: \
             the report carries no share of the network limits, `--json` has no \
             share fields, and no flag configures a share threshold"
        );
    }
}
