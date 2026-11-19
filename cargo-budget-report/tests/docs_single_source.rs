use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cargo-budget-report has a parent directory")
        .to_path_buf()
}

fn markdown_files(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|error| {
        panic!(
            "cannot read documentation directory {}: {error}",
            dir.display()
        )
    }) {
        let entry = entry.unwrap_or_else(|error| panic!("cannot read directory entry: {error}"));
        let path = entry.path();
        if path.is_dir() {
            markdown_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "md") {
            files.push(path);
        }
    }
}

#[test]
fn compatibility_matrix_has_one_canonical_source() {
    let docs_dir = repo_root().join("docs/src");
    let canonical = docs_dir.join("reference.md");
    let matrix_header = "| SDK Version | Protocol Version | Status | Notes |";
    let mut files = Vec::new();
    markdown_files(&docs_dir, &mut files);

    let matrix_files: Vec<PathBuf> = files
        .into_iter()
        .filter(|path| {
            fs::read_to_string(path)
                .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
                .contains(matrix_header)
        })
        .collect();

    assert_eq!(
        matrix_files,
        vec![canonical.clone()],
        "the compatibility matrix must be defined only in {}",
        canonical.display()
    );

    let contributing =
        fs::read_to_string(docs_dir.join("contributing.md")).expect("read contributing guide");
    assert!(
        contributing.contains("reference.md#supported-versions-compatibility"),
        "contributing guide must link to the canonical compatibility reference"
    );
}
