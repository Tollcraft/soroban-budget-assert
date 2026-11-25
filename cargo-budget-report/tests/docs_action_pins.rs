//! Documentation workflow snippets must use immutable GitHub Action pins.

use std::fs;
use std::path::PathBuf;

fn integration_guide() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .join("docs/src/ci_cd_integration.md")
}

#[test]
fn workflow_actions_are_pinned_to_full_commit_shas() {
    let path = integration_guide();
    let contents = fs::read_to_string(&path).expect("read CI/CD integration guide");
    let mut checked = 0;

    for (line_number, line) in contents.lines().enumerate() {
        let Some(uses) = line.split_once("uses:") else {
            continue;
        };
        let reference = uses.1.split_whitespace().next().unwrap_or_default();
        let Some((_, pin)) = reference.rsplit_once('@') else {
            panic!("{path:?}:{} has an invalid uses directive", line_number + 1);
        };
        assert!(
            pin.len() == 40 && pin.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "{path:?}:{} uses mutable action reference {reference:?}",
            line_number + 1
        );
        assert!(
            line.contains('#'),
            "{path:?}:{} should explain the version represented by {reference:?}",
            line_number + 1
        );
        checked += 1;
    }

    assert!(
        checked > 0,
        "expected workflow action references in {path:?}"
    );
}
