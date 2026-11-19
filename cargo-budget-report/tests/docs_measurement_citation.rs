use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cargo-budget-report has a parent directory")
        .to_path_buf()
}

#[test]
fn user_guide_qualifies_cost_figures_with_their_measured_profile() {
    let root = repo_root();
    let user_guide = fs::read_to_string(root.join("docs/src/user_guide.md"))
        .expect("read user guide");
    let measurements =
        fs::read_to_string(root.join("MEASUREMENTS.md")).expect("read measurements");
    let expected_example =
        "size-optimized release profile: local WASM ~901,816; testnet ~756,678";

    assert!(
        user_guide.contains(expected_example),
        "user guide must identify the profile associated with its example costs"
    );
    assert!(
        measurements.lines().any(|line| {
            line.contains("901,816") && line.contains("756,678") && line.contains("size-opt")
        }),
        "the cited values must appear together in MEASUREMENTS.md for the size-optimized profile"
    );
}
