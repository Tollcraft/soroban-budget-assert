//! `site/scripts/validate-anchors.sh` resolves the `#fragment` half of site
//! links. `validate.sh`'s file-existence check stops at the `#`, so a renamed
//! `id` used to deploy broken in-page navigation.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cargo-budget-report has a parent directory")
        .to_path_buf()
}

fn run_validator(dir: &Path) -> std::process::Output {
    Command::new("bash")
        .arg(repo_root().join("site/scripts/validate-anchors.sh"))
        .arg(dir)
        .output()
        .expect("run validate-anchors.sh")
}

#[test]
fn current_site_anchors_all_resolve() {
    let output = run_validator(&repo_root().join("site"));
    assert!(
        output.status.success(),
        "site anchors should resolve:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn same_page_and_cross_page_fragments_resolve() {
    let dir = tempfile::tempdir().expect("temp dir");
    fs::write(
        dir.path().join("index.html"),
        concat!(
            "<!DOCTYPE html><html><body>",
            "<nav><a href=\"#quickstart\">start</a>",
            "<a href=\"dashboard.html#controls\">controls</a></nav>",
            "<section id=\"quickstart\">go</section>",
            "</body></html>\n",
        ),
    )
    .expect("write index");
    fs::write(
        dir.path().join("dashboard.html"),
        concat!(
            "<!DOCTYPE html><html><body>",
            "<a name=\"legacy-anchor\"></a>",
            "<div id=\"controls\">c</div>",
            "<a href=\"#legacy-anchor\">legacy</a>",
            "</body></html>\n",
        ),
    )
    .expect("write dashboard");

    let output = run_validator(dir.path());
    assert!(
        output.status.success(),
        "expected both anchors to resolve:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn renamed_id_fails_with_source_line_and_destination() {
    let dir = tempfile::tempdir().expect("temp dir");
    fs::write(
        dir.path().join("index.html"),
        "<!DOCTYPE html><html><body>\n<a href=\"dashboard.html#controls\">c</a>\n</body></html>\n",
    )
    .expect("write index");
    fs::write(
        dir.path().join("dashboard.html"),
        "<!DOCTYPE html><html><body>\n<div id=\"filters\">c</div>\n</body></html>\n",
    )
    .expect("write dashboard");

    let output = run_validator(dir.path());
    assert!(
        !output.status.success(),
        "a missing anchor must fail validation"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("index.html:2"),
        "error should name the source file and line: {stderr}"
    );
    assert!(
        stderr.contains("#controls") && stderr.contains("dashboard.html"),
        "error should name the fragment and destination: {stderr}"
    );
}

#[test]
fn external_and_empty_fragments_are_ignored() {
    let dir = tempfile::tempdir().expect("temp dir");
    fs::write(
        dir.path().join("index.html"),
        concat!(
            "<!DOCTYPE html><html><body>",
            "<a href=\"#\">top</a>",
            "<a href=\"https://example.com/#nope\">external</a>",
            "<a href=\"missing.html#nope\">missing file</a>",
            "</body></html>\n",
        ),
    )
    .expect("write page");

    let output = run_validator(dir.path());
    assert!(
        output.status.success(),
        "bare `#`, external URLs and missing files are not anchor failures: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
