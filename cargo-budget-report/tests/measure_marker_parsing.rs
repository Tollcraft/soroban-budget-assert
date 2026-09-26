//! End-to-end tests for how `scripts/regenerate-measurements.sh` parses the
//! `// @measure <mode>[:<feature>[:<test_name>]]` markers.
//!
//! Every marker in the tree carries a trailing
//! `# discovered by scripts/regenerate-measurements.sh` comment. Folding that
//! comment into `<mode>` makes `"$mode" == "local"` fail for every harness
//! without a `:` in its marker, so the script reports
//! `SKIPPED (testnet required)` for work it was supposed to run, and the
//! markers with a `:` run with the comment glued onto the feature name.
//!
//! The script is driven against a scratch tree with a stub `cargo` on `PATH`,
//! so the assertions are about the argv the script builds, not about any real
//! contract being compiled.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Returns the absolute path to the real repository root (where `scripts/`
/// lives), regardless of the cwd the test binary is invoked from.
fn repo_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .expect("cargo-budget-report has a parent directory")
        .to_path_buf()
}

/// Returns a bash binary path that supports `mapfile`, which the
/// `regenerate-measurements.sh` script uses to collect the harness list.
///
/// macOS ships bash 3.2 as `/bin/bash`, where `mapfile` is missing and
/// the script aborts before it ever parses a marker. This function looks
/// for a newer bash instead of asserting against one that cannot run the
/// script at all.
fn bash_with_mapfile() -> Option<PathBuf> {
    for candidate in ["bash", "/opt/homebrew/bin/bash", "/usr/local/bin/bash"] {
        let ok = Command::new(candidate)
            .args(["-c", "type mapfile >/dev/null 2>&1"])
            .status()
            .map(|status| status.success())
            .unwrap_or(false);
        if ok {
            return Some(PathBuf::from(candidate));
        }
    }
    None
}

/// Lays out a git work tree holding one crate whose `tests/` directory
/// contains a harness per marker, a copy of the real `regenerate-measurements.sh`
/// script, and a stub `cargo` that records the argv it is handed.
///
/// The stub `cargo` writes each invocation's arguments to `$CARGO_LOG`
/// so the tests can verify the script built the correct argv for each
/// harness. The files are added to the git index (not committed) so
/// that `git ls-files` can discover them.
fn setup_scratch_tree(dir: &Path, markers: &[(&str, &str)]) {
    let tests = dir.join("scratch-crate").join("tests");
    fs::create_dir_all(&tests).unwrap();
    for (test_target, marker) in markers {
        fs::write(
            tests.join(format!("{test_target}.rs")),
            format!("{marker}\n\n#[test]\nfn placeholder() {{}}\n"),
        )
        .unwrap();
    }

    let scripts = dir.join("scripts");
    fs::create_dir_all(&scripts).unwrap();
    fs::copy(
        repo_root()
            .join("scripts")
            .join("regenerate-measurements.sh"),
        scripts.join("regenerate-measurements.sh"),
    )
    .unwrap();

    // The script discovers harnesses with `git ls-files`, which reads the
    // index, so the files need to be added but not committed.
    let bin = dir.join("stub-bin");
    fs::create_dir_all(&bin).unwrap();
    let cargo = bin.join("cargo");
    fs::write(
        &cargo,
        "#!/bin/sh\n{\n  printf 'INVOCATION\\n'\n  for a in \"$@\"; do printf 'ARG:%s\\n' \"$a\"; done\n} >> \"$CARGO_LOG\"\nexit 0\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&cargo, fs::Permissions::from_mode(0o755)).unwrap();
    }

    for args in [vec!["init", "-q"], vec!["add", "-A"]] {
        let status = Command::new("git")
            .args(&args)
            .current_dir(dir)
            .status()
            .expect("git should be available");
        assert!(status.success(), "git {args:?} failed");
    }
}

/// Holds the stdout from the script run and the path to the cargo
/// invocation log, which records all `cargo test` invocations made by
/// the script.
struct Run {
    stdout: String,
    cargo_log: String,
}

/// Runs the `regenerate-measurements.sh` script in the scratch tree
/// at `dir` using the given `bash` binary, and returns the combined
/// stdout and the cargo invocation log.
///
/// The `PATH` is modified to put the stub `cargo` first so the script
/// invokes our stub rather than the real cargo.
fn run_script(dir: &Path, bash: &Path) -> Run {
    let cargo_log = dir.join("cargo-invocations.log");
    let path = format!(
        "{}:{}",
        dir.join("stub-bin").display(),
        std::env::var("PATH").unwrap_or_default()
    );

    let output = Command::new(bash)
        .arg("scripts/regenerate-measurements.sh")
        .arg("--out")
        .arg(dir.join("out"))
        .current_dir(dir)
        .env("PATH", path)
        .env("CARGO_LOG", &cargo_log)
        .output()
        .expect("bash should run the regeneration script");

    Run {
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        cargo_log: fs::read_to_string(&cargo_log).unwrap_or_default(),
    }
}

/// Extracts the argv of the `cargo test` invocation for a given harness
/// from the cargo log as recorded by the stub `cargo`.
///
/// The log contains blocks separated by "INVOCATION\n", each with
/// "ARG:" prefixed lines. This function finds the block that starts
/// with "test" and includes the given `test_target`, then returns
/// the arguments as a vector of strings.
fn test_invocation(cargo_log: &str, test_target: &str) -> Vec<String> {
    for block in cargo_log.split("INVOCATION\n") {
        let args: Vec<String> = block
            .lines()
            .filter_map(|line| line.strip_prefix("ARG:").map(str::to_owned))
            .collect();
        if args.first().map(String::as_str) == Some("test")
            && args.iter().any(|arg| arg == test_target)
        {
            return args;
        }
    }
    Vec::new()
}

/// Asserts that a `local` marker with a trailing comment is still
/// parsed as mode `local` and not skipped.
#[test]
fn marker_with_trailing_comment_still_runs_locally() {
    let Some(bash) = bash_with_mapfile() else {
        eprintln!("no bash with `mapfile` available; skipping");
        return;
    };
    let tmp = tempfile::tempdir().expect("create tempdir");
    let dir = tmp.path();
    setup_scratch_tree(
        dir,
        &[(
            "budget_test",
            "// @measure local  # discovered by scripts/regenerate-measurements.sh",
        )],
    );

    let run = run_script(dir, &bash);

    assert!(
        !run.stdout.contains("SKIPPED"),
        "a `local` harness must not be skipped because of the marker's \
         trailing comment.\nstdout: {}",
        run.stdout
    );
    assert!(
        run.stdout
            .contains("Running scratch-crate budget_test (local)"),
        "expected the mode to parse as exactly `local`, got: {}",
        run.stdout
    );

    let args = test_invocation(&run.cargo_log, "budget_test");
    assert!(
        !args.is_empty(),
        "expected a `cargo test` invocation, log was:\n{}",
        run.cargo_log
    );
    assert!(
        !args.iter().any(|arg| arg == "--features"),
        "a marker with no `:` carries no feature, got: {args:?}"
    );
}

/// Asserts that the feature extracted from a marker like `local:sdk22`
/// does not include the trailing comment that follows the marker.
#[test]
fn marker_feature_excludes_the_trailing_comment() {
    let Some(bash) = bash_with_mapfile() else {
        eprintln!("no bash with `mapfile` available; skipping");
        return;
    };
    let tmp = tempfile::tempdir().expect("create tempdir");
    let dir = tmp.path();
    setup_scratch_tree(
        dir,
        &[(
            "calibrate_gap_sdk22",
            "// @measure local:sdk22  # discovered by scripts/regenerate-measurements.sh",
        )],
    );

    let run = run_script(dir, &bash);
    let args = test_invocation(&run.cargo_log, "calibrate_gap_sdk22");

    let feature = args
        .iter()
        .position(|arg| arg == "--features")
        .and_then(|idx| args.get(idx + 1))
        .unwrap_or_else(|| {
            panic!(
                "expected `--features <name>`, got: {args:?}\nlog:\n{}",
                run.cargo_log
            )
        });
    assert_eq!(
        feature, "sdk22",
        "the feature must not carry the marker's trailing comment"
    );
}

/// Asserts that a marker with three colon-separated fields correctly
/// splits into mode, feature, and test_name, with the test_name
/// passed as the harness filter.
#[test]
fn marker_splits_feature_and_test_name() {
    let Some(bash) = bash_with_mapfile() else {
        eprintln!("no bash with `mapfile` available; skipping");
        return;
    };
    let tmp = tempfile::tempdir().expect("create tempdir");
    let dir = tmp.path();
    setup_scratch_tree(
        dir,
        &[(
            "targeted",
            "// @measure local:sdk20:only_this_one  # discovered by x",
        )],
    );

    let run = run_script(dir, &bash);
    let args = test_invocation(&run.cargo_log, "targeted");

    let feature = args
        .iter()
        .position(|arg| arg == "--features")
        .and_then(|idx| args.get(idx + 1))
        .unwrap_or_else(|| panic!("expected `--features <name>`, got: {args:?}"));
    assert_eq!(feature, "sdk20");
    assert_eq!(
        args.last().map(String::as_str),
        Some("only_this_one"),
        "the test-name field should be passed as the harness filter, got: {args:?}"
    );
}

/// Asserts that a bare marker (no colon) carries no `--features` flag
/// and no test filter, avoiding the pitfall where `cut -f2` echoes
/// the whole string when the delimiter is absent.
#[test]
fn bare_marker_carries_no_feature_or_test_filter() {
    let Some(bash) = bash_with_mapfile() else {
        eprintln!("no bash with `mapfile` available; skipping");
        return;
    };
    let tmp = tempfile::tempdir().expect("create tempdir");
    let dir = tmp.path();
    setup_scratch_tree(dir, &[("plain", "// @measure local")]);

    let run = run_script(dir, &bash);
    let args = test_invocation(&run.cargo_log, "plain");

    assert!(
        !args.iter().any(|arg| arg == "--features"),
        "`cut -f2` echoes the whole field when there is no `:`, which would \
         invoke the harness as `--features local`. Got: {args:?}"
    );
    assert_eq!(
        args.last().map(String::as_str),
        Some("--nocapture"),
        "a bare marker names no test, so nothing should follow `--nocapture`. \
         Got: {args:?}"
    );
}

/// Asserts that a `testnet` marker is still skipped by the script
/// and no `cargo test` invocation is produced for it.
#[test]
fn testnet_marker_is_still_skipped() {
    let Some(bash) = bash_with_mapfile() else {
        eprintln!("no bash with `mapfile` available; skipping");
        return;
    };
    let tmp = tempfile::tempdir().expect("create tempdir");
    let dir = tmp.path();
    setup_scratch_tree(
        dir,
        &[(
            "network_only",
            "// @measure testnet  # discovered by scripts/regenerate-measurements.sh",
        )],
    );

    let run = run_script(dir, &bash);

    assert!(
        run.stdout.contains("SKIPPED (testnet required)"),
        "a `testnet` harness must still be skipped, got: {}",
        run.stdout
    );
    assert!(
        test_invocation(&run.cargo_log, "network_only").is_empty(),
        "a `testnet` harness must not be run, log:\n{}",
        run.cargo_log
    );
}
