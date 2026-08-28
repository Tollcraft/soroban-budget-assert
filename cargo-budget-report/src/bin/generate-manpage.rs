use clap_mangen::Man;
use std::fs;
use std::path::{Path, PathBuf};

#[path = "../cli.rs"]
mod cli;

use cli::BudgetReportArgs;

fn main() {
    let output = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("cargo-budget-report.1"));

    // Extract the BudgetReportArgs subcommand to render its full documentation
    let mut command = <BudgetReportArgs as clap::CommandFactory>::command();
    command = command.name("cargo-budget-report");
    command = command.bin_name("cargo-budget-report");

    let mut rendered = Vec::new();
    Man::new(command)
        .render(&mut rendered)
        .expect("failed to render generated man page");

    let output_path = Path::new(&output);

    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).expect("failed to create man-page output directory");
        }
    }

    fs::write(output_path, rendered).expect("failed to write generated man page");
}
