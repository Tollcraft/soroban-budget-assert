# Contributing to Soroban Budget Assert

First off, thank you for considering contributing to `soroban-budget-assert`! 

## How Can I Contribute?

### Reporting Bugs
- Ensure the bug was not already reported by searching on GitHub under Issues.
- If you're unable to find an open issue addressing the problem, open a new one.

### Suggesting Enhancements
- Open a new issue with a clear title and description.
- Provide as much context as possible, including why the enhancement is needed.

### Pull Requests
1. Fork the repo and create your branch from `main`.
2. If you've added code that should be tested, add tests.
3. If you've changed APIs, update the documentation.
4. Add a changelog entry in `CHANGELOG.md` for any user-visible change.
5. Ensure the test suite passes.
6. Issue that pull request!

## Local Development

### Unix (Linux / macOS)
- Install Rust and the Soroban CLI. The repository includes a `rust-toolchain.toml` file, so `rustup` will automatically install and use the correct toolchain and target when you run cargo commands.
- Run `cargo test` in the workspace root to run macro tests.
- Run `cargo run --bin cargo-budget-report` (or `cargo build`) to test the CLI locally.

### Windows (PowerShell)
- Install Rust by downloading and running [rustup-init.exe](https://rustup.rs/). Open a **new** PowerShell window, then add the WASM target:
  ```powershell
  rustup target add wasm32-unknown-unknown
  ```
- Install the Stellar CLI:
  ```powershell
  cargo install --locked stellar-cli
  ```
- Run `cargo test` in the workspace root to run macro tests.
- Run `cargo run --bin cargo-budget-report` (or `cargo build`) to test the CLI locally.

## Code Quality Standards
Before submitting a pull request, please ensure your code meets our quality standards by running the following commands locally:
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`

Please follow the styling and architectural patterns already used in the codebase.

### Pre-commit hook

To catch formatting issues automatically before they reach CI, install the
repository's pre-commit hook once after cloning:

```bash
bash scripts/install-hooks.sh
```

This runs `cargo fmt --all -- --check` before every commit and blocks the
commit if formatting is off. Fix with `cargo fmt --all` and commit again.
The hook only checks formatting — clippy and tests are intentionally left
to CI and the manual pre-PR checklist above, since they take longer to run.
