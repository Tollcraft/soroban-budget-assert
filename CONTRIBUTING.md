# Contributing to Soroban Budget Assert

First off, thank you for considering contributing to `soroban-budget-assert`! 

## How Can I Contribute?

### Reporting Bugs
- Ensure the bug was not already reported by searching on GitHub under Issues.
- If you're unable to find an open issue addressing the bug, open a new one.

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
- Install Rust and the Soroban CLI. The repository includes a `rust-toolchain.toml` file, so `rustup` will automatically install and use the correct toolchain and target when you run cargo commands.
- Run `cargo test` in the workspace root to run macro tests.
- Run `cargo run -p cargo-budget-report -- budget-report` (or `cargo build`) to test the CLI locally.

## Code Quality Standards
Before submitting a pull request, please ensure our quality standards by running the following commands locally:
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`

Please follow the styling and architectural patterns already used in the codebase.

## Repository configuration

Repository topics and branch protection are tracked in [`.github/settings.yml`](.github/settings.yml).
The configuration uses the Probot Settings application to apply repository settings from this
file. A repository administrator must install and authorize the Settings application for this
repository with permission to administer repository settings, then apply the configuration from
the default branch.

The configuration maintains the following topics: `soroban`, `stellar`, `rust`, `blockchain`,
`github-actions`, and `developer-tools`. The `main` branch requires the `Quality Checks` status
check, one approving review, linear history, and resolved conversations. Force-pushes and branch
deletions are disabled.

To verify the protection settings, open a test pull request targeting `main` after applying the
configuration. Confirm that the `Quality Checks` check is required and that merging is blocked
until the check passes and an approval is recorded. Close the test pull request after verification.

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
