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

### All platforms
- Install Rust and the Soroban CLI. The repository includes a `rust-toolchain.toml` file, so `rustup` will automatically install and use the correct toolchain and target when you run cargo commands.
- Run `cargo test` in the workspace root to run macro tests.
- Run `cargo run -p cargo-budget-report -- budget-report` (or `cargo build`) to test the CLI locally.

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
rustup target add wasm32-unknown-unknown
```

Install the Stellar CLI:
```bash
cargo install --locked stellar-cli
```

### Windows

On Windows, you can develop using **PowerShell** or **Git Bash** (included with Git for Windows).

Install prerequisites:
1. Install [Rust](https://rustup.rs) — the `.exe` installer sets up `rustup` and adds it to your `PATH` automatically.
2. Install [Git for Windows](https://git-scm.com/download/win) — includes Git Bash.
3. Install the [Visual C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) or Visual Studio with the "Desktop development with C++" workload — required to compile native Rust crates like `stellar-cli`.

Open **PowerShell** (or Git Bash) and run:
```powershell
# Add the WASM target
rustup target add wasm32-unknown-unknown

# Install the Stellar CLI
cargo install --locked stellar-cli
```

Create and fund a testnet identity:
```powershell
stellar keys generate alice --network testnet --fund
```

Build the WASM contract and run tests:
```powershell
cargo build -p amm-pool-contract --release --target wasm32-unknown-unknown
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
