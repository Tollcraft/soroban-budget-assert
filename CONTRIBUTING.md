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
4. Add a changelog entry in `CHANGELOG.md` under the `## Unreleased` section for any user-visible change.
5. Ensure the test suite passes.
6. If you are adding a new cost metric, follow the [Adding a New Cost Metric](docs/src/adding_a_metric.md) guide to ensure all coordinated edits are made.
7. Issue that pull request!

## Local Development

### All platforms
- Install Rust and the Soroban CLI. The repository includes a `rust-toolchain.toml` file, so `rustup` will automatically install and use the correct toolchain and target when you run cargo commands.
- Run `cargo test --workspace` in the workspace root to run the full workspace test suite.
- Run `cargo run -p cargo-budget-report -- budget-report` (or `cargo build`) to test the CLI locally.

### Calibration Harnesses vs. Regular Tests

The test directories in `amm-pool-contract/tests/` contain both **regular tests** (which assert correctness) and **calibration harnesses** (which measure and record cost figures for `MEASUREMENTS.md`).

**Calibration harnesses** are marked `#[ignore]` and excluded from the default `cargo test` run. They exist to generate the numbers in `MEASUREMENTS.md`, not to check correctness. Each harness file includes a doc comment explaining:
- What it measures
- How to run it deliberately
- Where the output is recorded

**Regular tests** run by default and assert correctness. Both types can coexist in the same file, but harness functions are clearly marked.

**To run calibration harnesses:**

```bash
# Build the WASM contract first
cargo build --target wasm32v1-none --release -p amm-pool-contract

# Run a specific harness (e.g., calibrate_gap)
cargo test -p amm-pool-contract --test calibrate_gap -- --ignored --nocapture

# Run all calibration harnesses
cargo test -p amm-pool-contract --test 'calibrate_*' -- --ignored --nocapture
cargo test -p amm-pool-contract --test 'measure_*' -- --ignored --nocapture
```

When you add a new measurement harness, ensure it:
1. Has a doc comment (at module or crate level) explaining what it measures, how to run it, and where output goes
2. Marks all measurement functions with `#[ignore]`
3. Keeps any genuine correctness assertions in separate, non-ignored tests if they belong in the same file

## Documentation

The documentation site is built with [GitBook](https://www.gitbook.com/) and published from `docs/src/` via Git Sync.
Content is written in standard Markdown with GitBook-specific blocks (`{% hint %}`, `{% code title %}`).

Edits merged to `main` publish automatically — no CI step is involved. To add a new page, create it
under `docs/src/` and add an entry to `docs/src/SUMMARY.md`.

### Previewing docs locally

**For a quick preview of the Markdown source** (without GitBook-specific rendering), open any
`.md` file in VS Code and press `Ctrl+Shift+V`, or run a simple HTTP server from the project root:

```bash
npx serve docs/src
```

**For a full GitBook-style preview**, the legacy `gitbook-cli` can build the site locally if you're
willing to install it. Note that `gitbook-cli` is no longer actively maintained and may require
troubleshooting (Node.js 16 is known to work; newer versions may need the `graceful-fs` polyfill
patched). From the project root:

```bash
nvm install 16        # if not already installed
nvm use 16
npm install -g gitbook-cli
gitbook serve docs/src
```

This starts a live-reload preview server at `http://localhost:4000`.

### Git Sync publishing

The docs publish automatically when changes are merged to `main` — no manual deployment step
is needed. The `.gitbook.yaml` configuration at the repository root points GitBook at `./docs/src/`
with `README.md` as the landing page and `SUMMARY.md` as the table of contents.

### Linux / macOS

Install system dependencies:
```bash
# Debian/Ubuntu
sudo apt-get install -y libdbus-1-dev pkg-config libudev-dev

# macOS (Homebrew)
brew install pkg-config dbus
```

Add the WASM target:
```bash
rustup target add wasm32v1-none
```

Install the Stellar CLI:
```bash
cargo install --locked stellar-cli
```

> `cargo-budget-report` still shells out to the `stellar` CLI for contract
> deploy and invoke-build (checked at preflight). Moving these to native RPC
> calls is tracked in [#123](https://github.com/Tollcraft/soroban-budget-assert/issues/123);
> `--source-secret` / `STELLAR_SECRET_KEY` is the signing-key mechanism that
> change will use.

### Windows

On Windows, you can develop using **PowerShell** or **Git Bash** (included with Git for Windows).

Install prerequisites:
1. Install [Rust](https://rustup.rs) — the `.exe` installer sets up `rustup` and adds it to your `PATH` automatically.
2. Install [Git for Windows](https://git-scm.com/download/win) — includes Git Bash.
3. Install the [Visual C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) or Visual Studio with the "Desktop development with C++" workload — required to compile native Rust crates like `stellar-cli`.

Open **PowerShell** (or Git Bash) and run:
```powershell
# Add the WASM target
rustup target add wasm32v1-none

# Install the Stellar CLI
cargo install --locked stellar-cli
```

Create and fund a testnet identity:
```powershell
stellar keys generate alice --network testnet --fund
```

Run the tests. The contract WASM is built automatically when missing or stale, so no manual `cargo build` step is needed:
```powershell
cargo build -p amm-pool-contract --release --target wasm32v1-none
cargo test --workspace
```

#### PATH setup

After installing with `cargo install`, Cargo's binary directory is usually at `%USERPROFILE%\.cargo\bin`. The Rust installer adds this to `PATH` automatically, but if `stellar` or `cargo` is not found:

```powershell
# Check if the directory is on PATH
$env:PATH -split ';' | Select-String '.cargo'

# Add it permanently for the current user (run as Administrator for machine-wide)
[Environment]::SetEnvironmentVariable(
    "PATH",
    "$env:PATH;$env:USERPROFILE\.cargo\bin",
    [EnvironmentVariableTarget]::User
)

# Restart your terminal, then verify
stellar --version
cargo --version
```

## Code Quality Standards & Lint/Formatting Configuration

This workspace configures both `clippy.toml` and `rustfmt.toml` at the repository root to enforce consistent code styling and static analysis rules across all development environments and CI pipelines.

### Linting (`clippy.toml`)
- **Purpose**: `clippy.toml` pins tunable lint thresholds (such as `too-many-arguments-threshold = 7`, `type-complexity-threshold = 250`, `enum-variant-name-threshold = 3`, and `single-char-binding-names-threshold = 4`) to upstream defaults.
- **Rationale**: Freezing these thresholds ensures that future Rust/Clippy toolchain updates will not trigger unexpected lint failures on unchanged source code.

### Formatting (`rustfmt.toml`)
- **Purpose**: `rustfmt.toml` specifies workspace code style rules, including `max_width = 100`, Unix line endings (`newline_style = "Unix"`), 4-space indentation (`tab_spaces = 4`), import reordering, and derive merging.
- **Toolchain Note**: The repository pins stable `1.91.0` in `rust-toolchain.toml`. Several configured options (`style_edition`, `force_explicit_abi`, `fn_params_layout`, `match_arm_leading_pipes`, `use_field_init_shorthand`, `use_try_shorthand`) are nightly-only in `rustfmt` and are silently ignored when running `cargo fmt` on the pinned stable 1.91.0 toolchain.

### Editor Setup
Most Rust-enabled editors (such as VS Code with `rust-analyzer`) automatically detect `rustfmt.toml` and `clippy.toml` at the workspace root when formatting on save. If your editor formats using a different style, ensure rust-analyzer is configured to use cargo/workspace commands (`"rust-analyzer.rustfmt.overrideCommand": ["cargo", "fmt", "--", "--config-path", "rustfmt.toml"]`).

### Pre-Push Verification Checklist
Before submitting a pull request, run the following commands locally from the workspace root to ensure all quality gates pass:
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

`cargo test --workspace` works on a clean checkout: the WASM-dependent tests
build the contract artifact on demand via the shared helper in
`amm-pool-contract/tests/common/` (issue #499), so a separate manual
`cargo build` is never required before testing.

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
until the check passes and an approval is recorded. Close the test pull request after verification.### Pre-commit hook

To catch formatting issues automatically before they reach CI, install the
repository's pre-commit hook once after cloning:

**Linux / macOS:**
```bash
bash scripts/install-hooks.sh
```

**Windows (PowerShell):**
```powershell
pwsh scripts/install-hooks.ps1
```

> On Windows you can also use Git Bash (included with Git for Windows) and
> run the `.sh` script: `bash scripts/install-hooks.sh`.

This runs `cargo fmt --all -- --check` before every commit and blocks the
commit if formatting is off. Fix with `cargo fmt --all` and commit again.
The hook only checks formatting — clippy and tests are intentionally left
to CI and the manual pre-PR checklist above, since they take longer to run.

## Regenerating measurements

The repository includes a discoverable script to regenerate all local measurement
harnesses in a single command. This is useful for keeping `MEASUREMENTS.md`
up‑to‑date after SDK upgrades or contract changes.

```bash
# Run all local measurement harnesses and capture their output.
# Results are written to ./measurements-out/ for review.
./scripts/regenerate-measurements.sh
```

The script:
- Scans `*/tests/*.rs` for the marker comment `// @measure <mode>[:<feature>[:<test_name>]]`.
- Runs harnesses marked as `local` (the default) and captures their stdout/stderr.
- Lists any `testnet` harnesses that require a funded Soroban testnet identity – these are
  skipped automatically; you can run them manually with the appropriate `stellar`
  CLI commands as documented in the corresponding measurement sections.

If you add a new measurement harness, include the marker comment at the top of the file.
The format is `// @measure <mode>[:<feature>[:<test_name>]]`:

- `// @measure local` – run automatically, no special features needed.
- `// @measure local:sdk20` – run with `--features sdk20`.
- `// @measure local:sdk22:calibrate_gap` – run with `--features sdk22`, only the named test.

The script will discover it without any additional configuration.

