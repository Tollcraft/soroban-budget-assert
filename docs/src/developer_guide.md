# Developer Guide

This guide is for developers who want to modify or extend `soroban-budget-assert`.

### Local Setup
1. Clone the repository.
2. Install Rust and the `stellar` CLI.
3. Configure a testnet identity named `alice` via `stellar keys generate alice --network testnet`.

### Workspace Structure
- `budget-macros`: Contains the procedural macros parsing the AST to inject budget assertions.
- `cargo-budget-report`: The CLI application utilizing `cargo_metadata` and `wasmparser`.
- `example-contract`: A sample Soroban contract used for integration testing.

### Testing
To test the macros, run:
```bash
cargo test
```
To test the CLI modifications, run:
```bash
cargo run --bin cargo-budget-report
```
