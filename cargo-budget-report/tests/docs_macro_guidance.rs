use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cargo-budget-report has a parent directory")
        .to_path_buf()
}

#[test]
fn contributing_guide_explains_macro_ui_and_runtime_tests() {
    let path = repo_root().join("docs/src/contributing.md");
    let guide = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));

    for required in [
        "budget-macros/tests/ui.rs",
        "budget-macros/tests/ui/",
        "TRYBUILD=overwrite cargo test -p budget-macros",
        "compiler diagnostics",
        "runtime tests",
        "amm-pool-contract",
    ] {
        assert!(
            guide.contains(required),
            "contributing guide is missing macro testing guidance `{required}`"
        );
    }
}
