//! Cross-check the shipped `tier-a-limits.env` against the keys the docs name.
//!
//! Issue #806: `docs/src/deriving_limits.md` and `README.md` document
//! `NETWORK__*` limits (and a `TIER_A__*` example) as living in
//! `tier-a-limits.env`. Copying a documented macro invocation panicked at
//! runtime whenever a documented key was missing from the file. These tests
//! fail if a key named in the docs ever drifts out of the shipped env file,
//! and if `tier-a-limits.provenance.md`'s table and the env keys stop
//! agreeing in both directions.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate lives one level below the repo root")
        .to_path_buf()
}

/// Parse `KEY=VALUE` lines of `tier-a-limits.env` (comments and blanks
/// ignored). Returns key -> value.
fn env_entries() -> Vec<(String, i64)> {
    let path = repo_root().join("tier-a-limits.env");
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let mut entries = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value
            .trim()
            .parse::<i64>()
            .unwrap_or_else(|e| panic!("bad value for {key}: {e}"));
        entries.push((key.to_string(), value));
    }
    entries
}

fn env_keys() -> BTreeSet<String> {
    env_entries().into_iter().map(|(k, _)| k).collect()
}

/// Collect every `TIER_A__…` / `NETWORK__…` key token named in the given
/// files. Wildcards such as `TIER_A__*` (nothing matches after `__`) are
/// skipped.
fn keys_named_in(docs: &[&str]) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for rel in docs {
        let path = repo_root().join(rel);
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        for prefix in ["TIER_A__", "NETWORK__"] {
            for (idx, _) in text.match_indices(prefix) {
                let rest = &text[idx + prefix.len()..];
                let suffix_len = rest
                    .chars()
                    .take_while(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || *c == '_')
                    .count();
                // Require a non-empty, complete token (skip `TIER_A__*`).
                if suffix_len > 0 {
                    found.insert(format!("{prefix}{}", &rest[..suffix_len]));
                }
            }
        }
    }
    found
}

/// Every `TIER_A__*` / `NETWORK__*` key named in `README.md` and
/// `docs/src/deriving_limits.md` must exist in the shipped env file.
#[test]
fn documented_keys_exist_in_tier_a_env() {
    let shipped = env_keys();
    let documented = keys_named_in(&["README.md", "docs/src/deriving_limits.md"]);
    assert!(
        !documented.is_empty(),
        "no TIER_A__*/NETWORK__* keys found in the docs - the scanner broke"
    );
    let missing: Vec<&String> = documented.iter().filter(|k| !shipped.contains(*k)).collect();
    assert!(
        missing.is_empty(),
        "keys named in README.md / docs/src/deriving_limits.md but absent from \
         tier-a-limits.env: {missing:?}"
    );
}

/// The four network limits that `docs/src/deriving_limits.md` tabulates must
/// be the four that ship, so the documented
/// `pct = N, of = env_file = "tier-a-limits.env", env = "NETWORK__…"` form
/// resolves instead of panicking on a missing-key lookup.
#[test]
fn network_limits_are_documented_and_shipped() {
    const NETWORK_KEYS: [&str; 4] = [
        "NETWORK__CPU",
        "NETWORK__MEM",
        "NETWORK__DISK_READ_BYTES",
        "NETWORK__DISK_WRITE_BYTES",
    ];
    let shipped = env_keys();
    for key in NETWORK_KEYS {
        assert!(
            shipped.contains(key),
            "{key} is documented in docs/src/deriving_limits.md but missing from \
             tier-a-limits.env"
        );
    }
    let docs = fs::read_to_string(repo_root().join("docs/src/deriving_limits.md")).unwrap();
    for key in NETWORK_KEYS {
        assert!(
            docs.contains(key),
            "{key} ships in tier-a-limits.env but is not tabulated in \
             docs/src/deriving_limits.md"
        );
    }
}

/// `tier-a-limits.provenance.md`'s source table and the `TIER_A__*` keys in
/// `tier-a-limits.env` must agree in both directions: every table row names a
/// shipped key with the shipped limit, every shipped key has a table row, and
/// every row satisfies the documented identity
/// `tier_a_limit = ceil(tier_b_value × margin)`.
#[test]
fn provenance_table_matches_env_in_both_directions() {
    let path = repo_root().join("tier-a-limits.provenance.md");
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));

    let mut rows: BTreeSet<String> = BTreeSet::new();
    let shipped: BTreeSet<String> = env_keys()
        .into_iter()
        .filter(|k| k.starts_with("TIER_A__"))
        .collect();

    for line in text.lines() {
        let line = line.trim();
        let Some(body) = line.strip_prefix('|').and_then(|l| l.strip_suffix('|')) else {
            continue;
        };
        let cells: Vec<&str> = body.split('|').map(|c| c.trim()).collect();
        if cells.len() != 4 {
            continue;
        }
        let key = cells[0].trim_matches('`');
        if !key.starts_with("TIER_A__") {
            continue;
        }
        let tier_b: f64 = cells[1]
            .parse()
            .unwrap_or_else(|e| panic!("row for {key}: bad tier_b {:?}: {e}", cells[1]));
        let margin: f64 = cells[2]
            .parse()
            .unwrap_or_else(|e| panic!("row for {key}: bad margin {:?}: {e}", cells[2]));
        let limit: i64 = cells[3]
            .parse()
            .unwrap_or_else(|e| panic!("row for {key}: bad limit {:?}: {e}", cells[3]));

        assert!(
            shipped.contains(key),
            "provenance table row `{key}` names a key that does not ship in \
             tier-a-limits.env"
        );
        assert_eq!(
            limit,
            env_limit(key),
            "provenance table limit for {key} ({limit}) disagrees with \
             tier-a-limits.env"
        );
        assert_eq!(
            limit,
            (tier_b * margin).ceil() as i64,
            "provenance row for {key} violates ceil(tier_b × margin) = limit"
        );
        rows.insert(key.to_string());
    }

    let untabled: Vec<&String> = shipped.iter().filter(|k| !rows.contains(*k)).collect();
    assert!(
        untabled.is_empty(),
        "keys shipped in tier-a-limits.env but absent from the \
         tier-a-limits.provenance.md table: {untabled:?}"
    );
}

fn env_limit(key: &str) -> i64 {
    env_entries()
        .into_iter()
        .find(|(k, _)| k == key)
        .unwrap_or_else(|| panic!("{key} missing from tier-a-limits.env"))
        .1
}
