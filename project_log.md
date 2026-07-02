# Soroban Budget Assert - Project Log

This document tracks the upgrades and changes made to graduate the project from an early-stage research prototype to a Minimum Viable Product (MVP) developer tool.

## Session Summary (Latest)

### 1. Codebase Polish and Linting
* Executed tests (`cargo test`) to ensure all base `budget-macros` logic and deliberate regressions worked successfully.
* Removed unused imports (`serde_json::Value`, `stellar_xdr::curr::Limits`, `Symbol`, `String`) and unnecessary `mut` bindings across the workspace.
* Ran `cargo clippy --fix` to address minor Rust idioms, including replacing unnecessary heap allocations (`Box::new`) in `budget-macros` and correcting generic borrows in the CLI.

### 2. Error Handling & Stability
* Refactored `cargo-budget-report/src/main.rs` to replace raw panics (`.expect()`, `.unwrap()`, `std::process::exit`) with the `anyhow` crate.
* Added `.context()` tracing for all network and CLI invocation failures, ensuring that developers receive graceful, readable errors (e.g., when testnet is down or a source account is unfunded) rather than crash dumps.

### 3. Centralized Configuration
* Introduced support for a `budget.toml` configuration file in the workspace root.
* Modified `cargo-budget-report` to optionally pull `network` and `source` variables directly from this file, drastically reducing the number of CLI flags developers have to type manually.

### 4. Continuous Integration (CI/CD)
* Updated `cargo-budget-report` to support a `--json` CLI flag, allowing output to be machine-readable instead of just human-readable terminal tables.
* Created `.github/workflows/budget.yml`, establishing a CI/CD GitHub Action that automatically tests the budget assertions and generates a report on PRs and pushes to `main`.

### 5. Automated Workspace Discovery (The "Cargo Test" Experience)
* Integrated `cargo_metadata` and `wasmparser` directly into the `cargo-budget-report` CLI.
* **The New Workflow:**
  * The CLI automatically scans the `Cargo.toml` workspace and finds all contracts (packages targeting `cdylib`).
  * It automatically builds those packages into `.wasm` targets.
  * It directly parses the `.wasm` bytecode to automatically discover all exported functions.
  * It iterates through every discovered function, injecting custom arguments defined in `budget.toml`, and simulates them against the network.
  * Finally, it aggregates all the data into a single master table reporting on the entire workspace at once.
* Fixed resulting Rust type mismatch errors (`CrateType`, `PackageName`) ensuring the tooling compiles perfectly.

### 6. Git & Repository Maintenance
* Created a `.gitignore` to prevent tracking of build artifacts (`target/`, `req.json`, `contract_id.txt`).
* Found and removed incorrectly nested `.git` directories in the workspace sub-crates that were preventing proper version control.
* Successfully committed all changes under `feat: upgrade to MVP with workspace discovery, budget.toml, and CI integration` and pushed directly to `main` on the Tollcraft repository.
