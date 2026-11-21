//! Regression tests for issue #968: `--derive-limits` must not lose Tier A
//! limits it cannot re-derive.
//!
//! Every test drives the real binary in a fresh tempdir (no `budget.toml`, so
//! the margins come from the flags) against the checked-in Tier B sample
//! report, and checks the three refusals — a key that would disappear, an
//! empty report, and a row with no `value` — plus the
//! `--allow-dropped-limits` escape hatch.

use assert_cmd::Command;
use std::path::{Path, PathBuf};

/// A key that exists in a previously-generated limits file but has no row in
/// the report, which is what a deleted contract function or a failed
/// simulation produces.
const RETIRED_KEY: &str = "TIER_A__AMM_POOL_CONTRACT__RETIRED_FN__CPU";

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn tier_b_sample() -> PathBuf {
    manifest_dir()
        .join("fixtures")
        .join("tier_b_report_sample.json")
}

fn derive_cmd(dir: &Path) -> Command {
    let mut cmd = Command::cargo_bin("cargo-budget-report").expect("binary should be built");
    cmd.current_dir(dir);
    cmd
}

fn margin_args() -> Vec<String> {
    vec![
        "--margin-cpu".to_string(),
        "1.5".to_string(),
        "--margin-memory".to_string(),
        "1.5".to_string(),
        "--margin-read".to_string(),
        "2".to_string(),
        "--margin-write".to_string(),
        "3".to_string(),
    ]
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

fn provenance_of(out_env: &Path) -> PathBuf {
    out_env.with_extension("provenance.md")
}

/// Derive a complete limits file once, so the next run has a real predecessor
/// to compare against.
fn seed_env(dir: &Path, out_env: &Path) {
    derive_cmd(dir)
        .args(["budget-report", "--derive-limits"])
        .arg(out_env)
        .arg("--from")
        .arg(tier_b_sample())
        .args(margin_args())
        .assert()
        .success();
    assert!(
        out_env.exists(),
        "the seeded derive should write {}",
        out_env.display()
    );
}

/// Seed the limits file and then add a key the report cannot produce, the way
/// a file written when the function still existed would look.
fn seed_env_with_retired_key(dir: &Path, out_env: &Path) -> String {
    seed_env(dir, out_env);
    let seeded = read(out_env);
    std::fs::write(out_env, format!("{seeded}{RETIRED_KEY}=987654\n")).unwrap();
    read(out_env)
}

#[test]
fn derive_refuses_to_drop_a_key_it_cannot_re_derive() {
    let dir = tempfile::tempdir().unwrap();
    let out_env = dir.path().join("tier-a-limits.env");
    let seeded = seed_env_with_retired_key(dir.path(), &out_env);
    let seeded_provenance = read(&provenance_of(&out_env));

    let assert = derive_cmd(dir.path())
        .args(["budget-report", "--derive-limits"])
        .arg(&out_env)
        .arg("--from")
        .arg(tier_b_sample())
        .args(margin_args())
        .assert()
        .failure();

    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).to_string();
    assert!(
        stderr.contains(RETIRED_KEY),
        "the refusal must name the dropped key, got: {stderr}"
    );
    assert!(
        stderr.contains("--allow-dropped-limits"),
        "the refusal must point at the opt-in flag, got: {stderr}"
    );

    // "byte-for-byte unchanged": neither output is touched, not even rewritten
    // with the same bytes.
    assert_eq!(read(&out_env), seeded, "the limits file must be untouched");
    assert_eq!(
        read(&provenance_of(&out_env)),
        seeded_provenance,
        "the provenance sidecar must be untouched"
    );
}

#[test]
fn allow_dropped_limits_writes_the_new_file_and_reports_the_removal() {
    let dir = tempfile::tempdir().unwrap();
    let out_env = dir.path().join("tier-a-limits.env");
    seed_env_with_retired_key(dir.path(), &out_env);

    let assert = derive_cmd(dir.path())
        .args(["budget-report", "--derive-limits"])
        .arg(&out_env)
        .arg("--from")
        .arg(tier_b_sample())
        .args(margin_args())
        .arg("--allow-dropped-limits")
        .assert()
        .success();

    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).to_string();
    assert!(
        stderr.contains(RETIRED_KEY),
        "the removal must be reported, got: {stderr}"
    );

    let written = read(&out_env);
    assert!(
        !written.contains(RETIRED_KEY),
        "the opt-in run must drop the key:\n{written}"
    );
    assert!(
        written.contains("TIER_A__AMM_POOL_CONTRACT__DEPOSIT__CPU"),
        "re-derivable limits must still be written:\n{written}"
    );
}

#[test]
fn an_empty_report_is_rejected_and_touches_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let out_env = dir.path().join("tier-a-limits.env");
    let empty = dir.path().join("empty.json");
    std::fs::write(&empty, r#"{"schema_version":1,"snapshots":[]}"#).unwrap();

    derive_cmd(dir.path())
        .args(["budget-report", "--derive-limits"])
        .arg(&out_env)
        .arg("--from")
        .arg(&empty)
        .args(margin_args())
        .assert()
        .failure()
        .stderr(predicates::str::contains("no measurement rows"));
    assert!(
        !out_env.exists(),
        "a rejected run must not create the limits file"
    );
    assert!(
        !provenance_of(&out_env).exists(),
        "a rejected run must not create the provenance sidecar"
    );

    // And with a predecessor present it is refused rather than truncated.
    let seeded = seed_env_with_retired_key(dir.path(), &out_env);
    derive_cmd(dir.path())
        .args(["budget-report", "--derive-limits"])
        .arg(&out_env)
        .arg("--from")
        .arg(&empty)
        .args(margin_args())
        .assert()
        .failure();
    assert_eq!(read(&out_env), seeded, "the limits file must be untouched");
}

#[test]
fn a_row_without_a_value_is_rejected_by_name() {
    let dir = tempfile::tempdir().unwrap();
    let out_env = dir.path().join("tier-a-limits.env");
    let partial = dir.path().join("partial.json");
    std::fs::write(
        &partial,
        r#"{"schema_version":1,"snapshots":[
            {"package":"amm-pool-contract","function":"deposit","metric":"CPU Instructions","value":30000},
            {"package":"amm-pool-contract","function":"retired_fn","metric":"CPU Instructions"}
        ]}"#,
    )
    .unwrap();

    let assert = derive_cmd(dir.path())
        .args(["budget-report", "--derive-limits"])
        .arg(&out_env)
        .arg("--from")
        .arg(&partial)
        .args(margin_args())
        .assert()
        .failure();

    let stderr = String::from_utf8_lossy(&assert.get_output().stderr).to_string();
    for needle in ["amm-pool-contract", "retired_fn", "CPU Instructions"] {
        assert!(
            stderr.contains(needle),
            "the error must name {needle:?}, got: {stderr}"
        );
    }
    assert!(
        !stderr.contains("missing field"),
        "the serde message named nothing; got: {stderr}"
    );
    assert!(!out_env.exists(), "nothing may be written on refusal");
}
