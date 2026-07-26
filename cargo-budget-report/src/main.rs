use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum OutputFormat {
    Text,
    Json,
}

#[derive(Debug, Parser)]
#[command(name = "cargo-budget-report")]
#[command(about = "Generate a report from Soroban budget snapshots")]
struct Cli {
    /// Output format.
    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    format: OutputFormat,

    /// Directory containing budget snapshots.
    #[arg(long, default_value = ".")]
    path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Snapshot {
    pub name: String,
    pub cpu: u64,
    pub memory: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BudgetReport {
    pub schema_version: u32,
    pub snapshots: Vec<Snapshot>,
}

impl BudgetReport {
    fn from_directory(path: &Path) -> Result<Self> {
        let mut snapshots = Vec::new();

        if path.is_dir() {
            for entry in fs::read_dir(path)
                .with_context(|| format!("failed to read {}", path.display()))?
            {
                let entry = entry?;
                let entry_path = entry.path();
                if entry_path.extension().and_then(|extension| extension.to_str()) != Some("json")
                {
                    continue;
                }

                let contents = fs::read_to_string(&entry_path).with_context(|| {
                    format!("failed to read snapshot {}", entry_path.display())
                })?;
                let snapshot: Snapshot = serde_json::from_str(&contents).with_context(|| {
                    format!("failed to parse snapshot {}", entry_path.display())
                })?;
                snapshots.push(snapshot);
            }
        }

        snapshots.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(Self {
            schema_version: 1,
            snapshots,
        })
    }

    fn render(&self, format: OutputFormat) -> Result<String> {
        match format {
            OutputFormat::Text => Ok(self
                .snapshots
                .iter()
                .map(|snapshot| {
                    format!(
                        "{}: cpu={}, memory={}",
                        snapshot.name, snapshot.cpu, snapshot.memory
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")),
            OutputFormat::Json => serde_json::to_string_pretty(self)
                .context("failed to serialize budget report as JSON"),
        }
    }
}

fn run(cli: Cli) -> Result<()> {
    let report = BudgetReport::from_directory(&cli.path)?;
    println!("{}", report.render(cli.format)?);
    Ok(())
}

fn main() {
    if let Err(error) = run(Cli::parse()) {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_output_has_the_versioned_report_schema() {
        let report = BudgetReport {
            schema_version: 1,
            snapshots: vec![Snapshot {
                name: "transfer".to_owned(),
                cpu: 123,
                memory: 456,
            }],
        };

        let value: serde_json::Value =
            serde_json::from_str(&report.render(OutputFormat::Json).unwrap()).unwrap();

        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["snapshots"][0]["name"], "transfer");
        assert_eq!(value["snapshots"][0]["cpu"], 123);
        assert_eq!(value["snapshots"][0]["memory"], 456);
        assert_eq!(
            value.as_object().unwrap().keys().collect::<Vec<_>>(),
            vec!["schema_version", "snapshots"]
        );
    }

    #[test]
    fn json_is_selected_by_the_format_flag() {
        let cli = Cli::try_parse_from(["cargo-budget-report", "--format", "json"]).unwrap();
        assert_eq!(cli.format, OutputFormat::Json);
    }
}
