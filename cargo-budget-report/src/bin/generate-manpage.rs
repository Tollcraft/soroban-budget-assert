//! Generates the `cargo-budget-report(1)` man page from the CLI definition.
//!
//! Usage: `generate-manpage [OUTPUT]` — writes the roff man page to `OUTPUT`
//! (default `cargo-budget-report.1` in the current directory), creating any
//! missing parent directories. The release workflow runs this to ship the page.
//!
//! The page is rendered from [`BudgetReportArgs`] itself, so it cannot drift
//! from the flags the tool actually accepts. Each step below is a small
//! function so it can be tested without spawning the binary or touching the
//! real working directory.

use clap::CommandFactory;
use clap_mangen::Man;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[path = "../cli/mod.rs"]
mod cli;

use cli::args::BudgetReportArgs;

/// Where the man page is written when no output path is given.
const DEFAULT_OUTPUT: &str = "cargo-budget-report.1";

/// Name shown in the man page's title and synopsis.
const COMMAND_NAME: &str = "cargo-budget-report";

fn main() {
    let output = output_path(std::env::args().nth(1));

    // The `expect` messages are the tool's user-facing failure text, and each
    // step keeps its own so a release log says which one failed.
    let rendered = render_man_page(command()).expect("failed to render generated man page");
    ensure_parent_dir(&output).expect("failed to create man-page output directory");
    fs::write(&output, rendered).expect("failed to write generated man page");
}

/// Resolves the destination from the first CLI argument, falling back to
/// [`DEFAULT_OUTPUT`].
///
/// An explicitly empty argument is passed through unchanged (and will fail at
/// write time) rather than silently replaced by the default.
fn output_path(arg: Option<String>) -> PathBuf {
    arg.map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_OUTPUT))
}

/// The `clap` command the page documents: the `budget-report` arguments,
/// presented as the standalone `cargo-budget-report` binary rather than as
/// `cargo`'s wrapper subcommand.
fn command() -> clap::Command {
    <BudgetReportArgs as CommandFactory>::command()
        .name(COMMAND_NAME)
        .bin_name(COMMAND_NAME)
}

/// Renders `command` to roff.
fn render_man_page(command: clap::Command) -> io::Result<Vec<u8>> {
    let mut rendered = Vec::new();
    Man::new(command).render(&mut rendered)?;
    Ok(rendered)
}

/// The directory that must exist before `path` can be written, if any.
///
/// A bare file name (`page.1`) has an empty parent, meaning the current
/// directory, which always exists.
fn parent_to_create(path: &Path) -> Option<&Path> {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
}

/// Creates the missing parent directories of `path`, if it has any.
fn ensure_parent_dir(path: &Path) -> io::Result<()> {
    match parent_to_create(path) {
        Some(parent) => fs::create_dir_all(parent),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_path_defaults_when_no_argument_is_given() {
        assert_eq!(output_path(None), PathBuf::from("cargo-budget-report.1"));
    }

    #[test]
    fn output_path_uses_the_argument_verbatim() {
        assert_eq!(
            output_path(Some("dist/man/page.1".to_string())),
            PathBuf::from("dist/man/page.1")
        );
    }

    #[test]
    fn output_path_does_not_replace_an_empty_argument_with_the_default() {
        assert_eq!(output_path(Some(String::new())), PathBuf::from(""));
    }

    #[test]
    fn command_is_named_after_the_standalone_binary() {
        let command = command();
        assert_eq!(command.get_name(), "cargo-budget-report");
        assert_eq!(command.get_bin_name(), Some("cargo-budget-report"));
    }

    #[test]
    fn command_definition_is_internally_consistent() {
        // Catches conflicting or dangling clap argument definitions.
        command().debug_assert();
    }

    #[test]
    fn rendered_page_is_roff_and_documents_the_real_flags() {
        let page = String::from_utf8(render_man_page(command()).unwrap()).unwrap();
        // The title macro takes the bare name; body text escapes `-` as `\-`.
        assert!(page.contains(".TH cargo-budget-report 1"), "{page}");
        assert!(page.contains(".SH NAME\ncargo\\-budget\\-report"), "{page}");
        for flag in [
            "\\-\\-check",
            "\\-\\-json",
            "\\-\\-rpc\\-url",
            "\\-\\-concurrency",
        ] {
            assert!(page.contains(flag), "man page is missing {flag}");
        }
    }

    #[test]
    fn rendering_is_deterministic() {
        assert_eq!(
            render_man_page(command()).unwrap(),
            render_man_page(command()).unwrap()
        );
    }

    #[test]
    fn parent_to_create_is_none_for_a_bare_file_name() {
        assert_eq!(parent_to_create(Path::new("page.1")), None);
    }

    #[test]
    fn parent_to_create_is_the_containing_directory() {
        assert_eq!(
            parent_to_create(Path::new("dist/man/page.1")),
            Some(Path::new("dist/man"))
        );
    }

    #[test]
    fn ensure_parent_dir_creates_nested_directories() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("a").join("b").join("page.1");
        ensure_parent_dir(&path).unwrap();
        assert!(path.parent().unwrap().is_dir());
        assert!(
            !path.exists(),
            "only the directory is created, not the file"
        );
    }

    #[test]
    fn ensure_parent_dir_is_a_no_op_for_a_bare_file_name() {
        ensure_parent_dir(Path::new("page.1")).unwrap();
    }

    #[test]
    fn ensure_parent_dir_is_idempotent_for_an_existing_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("page.1");
        ensure_parent_dir(&path).unwrap();
        ensure_parent_dir(&path).unwrap();
    }

    #[test]
    fn ensure_parent_dir_fails_when_a_parent_is_a_regular_file() {
        let tmp = tempfile::tempdir().unwrap();
        let blocker = tmp.path().join("blocker");
        fs::write(&blocker, "").unwrap();
        assert!(ensure_parent_dir(&blocker.join("page.1")).is_err());
    }
}
