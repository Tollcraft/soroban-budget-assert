# Developer Guide

This guide is for developers modifying or extending `soroban-budget-assert` itself.

## Local setup

### Linux / macOS

1. Clone the repository.
2. Install Rust with the WASM target: `rustup target add wasm32-unknown-unknown`.
3. Install the Stellar CLI: `cargo install --locked stellar-cli` (on Debian/Ubuntu, first `sudo apt-get install -y libdbus-1-dev pkg-config libudev-dev`).
4. Create and fund a testnet identity: `stellar keys generate alice --network testnet --fund`.

### Windows

1. Clone the repository.
2. Install [Rust](https://rustup.rs) — the `.exe` installer adds `rustup` and `cargo` to your `PATH`.
3. Install [Git for Windows](https://git-scm.com/download/win) — includes Git Bash for the pre-commit hook.
4. Install the [Visual C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) — required to compile `stellar-cli`.
5. Open **PowerShell** and run:

```powershell
# Add the WASM target
rustup target add wasm32-unknown-unknown

# Install the Stellar CLI
cargo install --locked stellar-cli

# Create and fund a testnet identity
stellar keys generate alice --network testnet --fund
```

6. Build the contract WASM and run tests:

```powershell
cargo build -p amm-pool-contract --release --target wasm32-unknown-unknown
cargo test --workspace
```

#### PATH troubleshooting

If `stellar` or `cargo` is not found after installation:

```powershell
# Check if ~/.cargo/bin is on PATH
$env:PATH -split ';' | Select-String '.cargo'

# Add it permanently (restart terminal afterward)
[Environment]::SetEnvironmentVariable(
    "PATH",
    "$env:PATH;$env:USERPROFILE\.cargo\bin",
    [EnvironmentVariableTarget]::User
)
```

## Workspace structure

| Crate | Role |
|---|---|
| `budget-macros` | Proc-macro crate. `budget_cpu_lt` / `budget_mem_lt` / `budget_lt` / `budget_write_bytes_lt` rewrite a test function's block so a budget assertion against its `env` variable runs on every exit path. They all go through `instrument_exit_paths()`; `ReturnRewriter` handles the `return` cases. |
| `cargo-budget-report` | The CLI (`cargo budget-report` subcommand). Uses `cargo_metadata` for workspace discovery, `wasmparser` for export scanning, shells out to `stellar` for deploy/invoke/XDR decode, and `tabled`/`serde_json` for output. |
| `amm-pool-contract` | Reference contract (`do_expensive_work`) plus the integration tests that double as the research measurements. |

`budget.toml` at the root configures the CLI for the example contract, and `.github/workflows/budget.yml` runs the Tier A tests in CI.

## Testing

The macro tests execute the compiled WASM, so build it first:

```bash
cargo build -p amm-pool-contract --release --target wasm32-unknown-unknown
cargo test
```

`amm-pool-contract/tests/budget_test.rs` covers the macros against the real SDK `Env` (32 `#[test]` functions total):

- `test_budget_raw_rust` / `test_budget_wasm` — print raw-Rust vs. WASM local cost estimates (the source of the measured-gap figures in Mechanics); `test_budget_wasm` asserts a 5,000,000 CPU limit via `#[budget_cpu_lt(5000000)]`.
- `test_budget_macro_gated` — a passing assertion at the 3,500,000 CPU limit (`#[budget_cpu_lt(3500000)]`).
- `test_budget_macro_deliberate_regression` — asserts an intentionally low limit (1,000,000) and expects the macro's panic (`#[budget_cpu_lt(1000000)]` with `#[should_panic]`).
- `test_budget_macro_dynamic_env` — asserts a CPU limit read from the `TEST_MAX_CPU` environment variable (`#[budget_cpu_lt(env = "TEST_MAX_CPU")]`).
- `test_budget_macro_dynamic_env_fallback` — verifies the fallback behaviour: when the env var is unset, the limit defaults to `u64::MAX` and the assertion passes unconditionally (`#[budget_cpu_lt(env = "TEST_MAX_CPU_FALLBACK")]`).
- `test_budget_macro_json_config_*` (6 tests) — the `config = "key"` limit form read from `budget.json`.
- `test_budget_require_auth_*` (7 tests) — CPU and memory budget assertions for `require_auth` calls covering isolated calls, deposit/swap/withdraw operations, and deliberate-regression cases.
- `test_budget_extend_ttl_*` (4 tests) — CPU and memory budget assertions for `extend_instance_ttl` calls with passing and deliberate-regression variants.
- `test_read_bytes_budget_*` / `test_write_bytes_*` (5 tests) — ledger read-bytes budget enforcement, write-bytes budget via `#[budget_write_bytes_lt]`, and deliberate-regression fixtures.
- `test_budget_macro_result_returning` / `test_budget_macro_result_returning_regression` / `test_budget_macro_early_return_still_asserts` — the `Result`-returning and early-`return` body shapes.
`amm-pool-contract/tests/budget_test.rs` covers the macros against the real SDK `Env`:

- `test_budget_raw_rust` / `test_budget_wasm` — print raw-Rust vs. WASM local cost estimates (the source of the measured-gap figures in Mechanics).
- `test_budget_macro_gated` — a passing assertion at the 950,000 CPU limit.
- `test_budget_macro_deliberate_regression` — asserts an intentionally low limit (600,000) and expects the macro's panic, proving the gate fires.
- `test_budget_macro_dynamic_env` — asserts a CPU limit read from the `TEST_MAX_CPU` environment variable.
- `test_budget_macro_dynamic_env_fallback` — verifies the fallback behaviour: when the env var is unset, the limit defaults to `u64::MAX` and the assertion passes unconditionally.
- `test_budget_macro_json_config_*` — the `config = "key"` limit form read from `budget.json`.
- `test_budget_macro_result_returning` / `..._regression` / `test_budget_macro_early_return_still_asserts` — the `Result`-returning and early-`return` body shapes.

`budget-macros/tests/ui.rs` is a `trybuild` suite that needs no WASM and no SDK. `tests/ui/*.rs` must fail to compile, with the diagnostic pinned in the matching `.stderr` — regenerate those with `TRYBUILD=overwrite cargo test -p budget-macros`. `tests/ui/pass/*.rs` must compile *and run*: each one exercises a test-body shape against the mock `env` in `tests/ui/support/mock_env.rs` (fixed costs) and asserts which cost and limit the injected check reports, so a body shape that silently stops being checked fails there.

To exercise the CLI end-to-end against testnet (requires the funded `alice` identity):

```bash
cargo run -p cargo-budget-report -- budget-report
```

## Extending

- **New assertion metrics** — follow the pattern in `budget-macros/src/lib.rs`: build the metric's `assert!` with its accessor on `env.cost_estimate().budget()`, then hand the function and that assertion to `instrument_exit_paths()` so the check reaches every exit path. Keep the failure message explicit. Add a passing test and a `#[should_panic]` regression test in `amm-pool-contract`, plus a `tests/ui/pass/` UI case.
- **CLI changes** — no panics; return `anyhow::Result` with `.context()` on every external call (network, `stellar` invocations, file I/O). Any new output must also work under `--json`.
- **Docs** — this site is GitBook, synced from the repository via Git Sync (`.gitbook.yaml` points at `docs/src`). Edits merged to `main` publish automatically; no CI step is involved. Add pages to `docs/src/SUMMARY.md` (GitBook's table of contents). GitBook-specific blocks (`{% hint %}`, `{% code title %}`) are available in any page.

## Docs site appearance

The site's look and feel is configured by a space admin in the GitBook app (**space → Customize**), not in this repository. The intended configuration:

- **Theme**: dark mode as the default, with the light/dark toggle enabled.
- **Accent color**: a single vibrant, high-contrast accent (used for links, hint borders, and active nav) against GitBook's deep dark background.
- **Code blocks**: syntax highlighting works from the fence language tags already present in these pages (`rust`, `bash`, `toml`, `json`); enable line numbers for long snippets if desired.

Content and structure changes belong in this repo; theme changes belong in the GitBook UI.

## Troubleshooting

This section covers common issues with **Stellar Friendbot** — the testnet faucet that funds source accounts for contract deployment and simulation.

### Overview

Friendbot is a free service that airdrops test XLM into Stellar testnet accounts. This project depends on a funded testnet account because `cargo budget-report` deploys contracts and runs simulations against the live testnet RPC endpoint (`https://soroban-testnet.stellar.org:443`). Each deploy and simulation consumes a small amount of testnet XLM from the source account.

You encounter Friendbot during initial setup when running:

```bash
stellar keys generate alice --network testnet --fund
```

And when re-funding an existing account after its balance has been depleted or reset:

```bash
stellar keys fund alice --network testnet
```

The `--fund` flag and the `fund` subcommand both call Friendbot under the hood.

### Common Errors

#### Friendbot unavailable

| | |
|---|---|
| **Symptoms** | `stellar keys generate alice --network testnet --fund` returns an HTTP 404, 503, or connection refused error. |
| **Cause** | Friendbot service is down or overloaded. This is shared infrastructure maintained by the Stellar Development Foundation. |
| **Resolution** | Check [Stellar System Status](https://status.stellar.org/). Wait a few minutes and retry. Friendbot availability is outside the project's control. |
| **Prevention** | None — this is an infrastructure-level issue. |

#### Network timeout

| | |
|---|---|
| **Symptoms** | Funding command hangs for 30+ seconds, then fails with a timeout error. `cargo budget-report` reports `failed to execute curl` or `Failed to parse RPC response`. |
| **Cause** | Network connectivity issues between your machine and Stellar infrastructure, or Friendbot rate-limiting. |
| **Resolution** | Retry the command. If the problem persists, check your network connection and confirm the RPC endpoint is reachable: `curl -s -o /dev/null -w "%{http_code}" https://soroban-testnet.stellar.org:443`. |
| **Prevention** | Use a reliable network connection. The CLI retries deploys automatically with exponential backoff (up to 4 attempts). |

#### Rate limiting

| | |
|---|---|
| **Symptoms** | Friendbot returns HTTP 429 or a "rate limit exceeded" message. |
| **Cause** | Too many funding requests from the same IP address in a short period. |
| **Resolution** | Wait 60 seconds and retry. If you need multiple accounts, generate them with spacing between requests. |
| **Prevention** | Fund accounts once and reuse them. One funded testnet identity is sufficient for this project's workflow. |

#### Account already exists on ledger

| | |
|---|---|
| **Symptoms** | Friendbot returns an error about the account already existing, or `stellar keys generate` succeeds but funding does not add new XLM. |
| **Cause** | The Stellar public key already has a testnet account with a balance. Friendbot will not fund it a second time. |
| **Resolution** | Check the account balance with `stellar keys show alice`. If the balance is sufficient (a few XLM is enough for deploy and simulation fees), proceed without additional funding. If the balance is too low, use `stellar keys fund alice --network testnet` instead. |
| **Prevention** | Check the account balance before requesting funding. |

#### Unfunded or reset testnet account

| | |
|---|---|
| **Symptoms** | `cargo budget-report` fails with `Ensure your source account is funded` or `Failed to deploy <package> after 4 attempts. Ensure your source account is funded.` in the error chain. Deployment errors may mention `txBadSeq` or `txInsufficientBalance`. |
| **Cause** | Friendbot-funded testnet accounts are reset periodically by network policy. Inactive accounts are wiped, and even active accounts may have their balances drained over time. The deploy step calls `stellar contract deploy` which implicitly relies on Friendbot on testnet; a depleted account causes this to fail. |
| **Resolution** | Re-fund the account: `stellar keys fund alice --network testnet`. If the account has been fully wiped, generate a fresh one: `stellar keys generate alice --network testnet --fund` (this overwrites the existing local keypair, so export the secret key first if needed). |
| **Prevention** | Re-fund the account after any period of project inactivity longer than a few days. If you are running `cargo budget-report` regularly, the existing account stays active. |

#### Invalid public key or source account

| | |
|---|---|
| **Symptoms** | Errors about "invalid public key", "invalid account ID", or `cargo budget-report` failing with `missing --source or budget.toml source field`. |
| **Cause** | The `source` field in `budget.toml` does not match any local Stellar keypair, or the keypair name is misspelled. |
| **Resolution** | List configured identities: `stellar keys ls`. Verify the configured source matches: `stellar keys show <name>`. Check `budget.toml` to ensure `source = "<name>"` uses the correct name (default is `"alice"`). |
| **Prevention** | Run `cargo budget-report --init` to scaffold a template `budget.toml` with the default values. |

#### Testnet vs Futurenet mismatch

| | |
|---|---|
| **Symptoms** | Transactions fail on testnet because the identity was generated with `--network futurenet`, or vice versa. `cargo budget-report` may return `stellar contract deploy failed` with protocol errors in stderr. |
| **Cause** | The source account was created on one network but `budget.toml` or the `--network` flag targets another. Accounts and Friendbot operate per-network. |
| **Resolution** | Ensure the network flag used during `stellar keys generate --fund` matches the `network` setting in `budget.toml`. Use `stellar keys ls` to verify which network a key belongs to. Re-generate the identity for the correct network if needed. |
| **Prevention** | Stay consistent: this project uses `testnet` by default. Do not pass `--network futurenet` unless you have explicitly configured for Futurenet. |

#### RPC endpoint unreachable

| | |
|---|---|
| **Symptoms** | `cargo budget-report` fails with `failed to execute curl`, `Failed to parse RPC response`, or `error` field in the simulateTransaction response. |
| **Cause** | The Soroban RPC endpoint (`https://soroban-testnet.stellar.org:443`) is unreachable, or the network configuration does not point to a valid endpoint. When `cargo budget-report --network local` is used without a local RPC server running, the tool falls back to documented protocol limits instead of live data. |
| **Resolution** | Verify the endpoint is reachable: `curl -s -o /dev/null -w "%{http_code}" https://soroban-testnet.stellar.org:443`. A healthy endpoint returns `200`. If the endpoint is down, wait and retry. For `--network local`, ensure you have a local Soroban RPC server running. |
| **Prevention** | Use the default `testnet` network in `budget.toml` unless you have a specific reason to use another network. |

#### Environment configuration missing

| | |
|---|---|
| **Symptoms** | `cargo budget-report` exits with `missing --network or budget.toml network field` or `missing --source or budget.toml source field`. |
| **Cause** | Neither `budget.toml` nor the corresponding CLI flag provides the required `network` or `source` value. |
| **Resolution** | Create `budget.toml` at the workspace root with `network = "testnet"` and `source = "alice"`, or pass `--network testnet --source alice` on the command line. Use `cargo budget-report --init` to generate a template. |
| **Prevention** | Run `cargo budget-report --init` as part of the initial project setup. Always commit `budget.toml` to version control. |

### Diagnostic checklist

Before opening an issue or asking for help, run through this checklist:

1. **Is the Stellar CLI installed?** → `stellar --version`. If missing, install with `cargo install --locked stellar-cli`.
2. **Is the source account configured?** → `stellar keys ls`. You should see your source identity (default: `alice`).
3. **Is the account funded?** → `stellar keys show alice`. The output includes the account balance. A balance of 0 or an "account does not exist" error means the account needs funding.
4. **Is `budget.toml` present with correct values?** → Check that `network = "testnet"` and `source = "alice"` (or your chosen values) are set at the workspace root.
5. **Is the RPC endpoint reachable?** → `curl -s -o /dev/null -w "%{http_code}" https://soroban-testnet.stellar.org:443`. Should return `200`.
6. **Has the account been idle for more than a few days?** → Friendbot-funded testnet accounts are periodically reset. Re-fund with `stellar keys fund alice --network testnet` before investigating further.
7. **Does the test build succeed?** → `cargo build -p amm-pool-contract --release --target wasm32-unknown-unknown`. A build failure produces the same red CI check as a budget regression but has nothing to do with Friendbot — the error message will clearly name the build issue.

### Recovery steps

**Re-fund the existing account** (fastest recovery for a depleted account):

```bash
stellar keys fund alice --network testnet
```

**Generate a fresh account** (use when the existing identity is corrupted, the secret key is lost, or you want a clean slate):

```bash
# Export the current secret key if you need to preserve it
stellar keys show alice --secret

# Generate a new identity and fund it (overwrites the local keypair)
stellar keys generate alice --network testnet --fund
```

**Update `budget.toml` with a different source account** (use when switching to a different identity):

```toml
network = "testnet"
source = "bob"  # changed from "alice"
```

**Reset the local environment** (use when a stale WASM build produces confusing numbers):

`cargo budget-report` keeps no cache of its own — every run rebuilds the WASM and deploys it from scratch, so there are no cached deploy artifacts to remove. The only stale state that can affect results is the Cargo build output in `target/`, which Cargo normally rebuilds incrementally and correctly. If you want a guaranteed-clean run anyway:

```bash
# Force a from-scratch WASM build (removes all build output)
cargo clean

# Rebuild and re-deploy
cargo budget-report
```

A full `cargo clean` also wipes host-side debug builds, so prefer `rm -rf target/wasm32-unknown-unknown` if you only want to invalidate the WASM artifacts. Note that each `cargo budget-report` run deploys a *new* contract instance on the target network; previously deployed contract IDs are not tracked or reused, so there is nothing to reset on that side.

### Best practices

- **Fund once, reuse.** One funded testnet identity is sufficient for local development and CI. Avoid creating new accounts for every session.
- **Keep `budget.toml` in version control.** The `network` and `source` defaults live in `budget.toml`. Committing it ensures every contributor and CI run uses the same configuration.
- **Verify the balance before debugging deeper issues.** Run `stellar keys show alice` before assuming a deploy failure is a code problem.
- **Re-fund after inactivity.** If the project has not been used for more than a week, re-fund the account before troubleshooting any `cargo budget-report` failure.
- **Check the preflight output.** `cargo budget-report` runs preflight checks that verify the Stellar CLI and WASM target are installed. Read the output before diving into Friendbot debugging.
- **Follow setup steps in order.** The [Local setup](#local-setup) sequence matters: install the Stellar CLI → generate the identity → fund it → configure `budget.toml` → run `cargo budget-report`. Skipping a step produces errors that look like Friendbot problems but are actually setup gaps.

### Additional resources

- [Stellar Friendbot documentation](https://developers.stellar.org/docs/network/faucet) — official Friendbot usage guide
- [Stellar System Status](https://status.stellar.org/) — check for ongoing Friendbot or RPC outages
- [Stellar CLI reference](https://github.com/stellar/stellar-cli) — all `stellar keys` subcommands
- [Stellar network information](https://developers.stellar.org/docs/network/) — testnet vs Futurenet vs pubnet differences
- The [CI Tutorial](ci_tutorial.md) has a [Troubleshooting](ci_tutorial.md#troubleshooting) section with CI-specific failure modes
- The [End-User Guide](user_guide.md) covers `budget.toml` configuration and first-time setup

## ⚙️ Supported Versions & Compatibility

* **Supported SDK Version**: `soroban-sdk` = `"22.0.11"` (specifically tested/resolved to `22.0.11` in `Cargo.lock`)
* **Supported XDR Version**: `stellar-xdr` = `"22.1.0"` (used for decoding transaction simulation responses)
* **Corresponding Stellar Protocol**: **Protocol 22**

### Compatibility Matrix

| SDK Version | Protocol Version | Status | Notes |
| :--- | :--- | :--- | :--- |
| **`< 22.0.0`** | `< 22` | **Untested** | Older protocols may use different transaction/resource schemas. |
| **`22.0.x`** | `22` | **Supported** | Matches pinned manifest dependencies (`soroban-sdk` `22.0.11`, `stellar-xdr` `22.1.0`). |
| **`>= 23.0.0`** | `>= 23` | **Untested** | Future protocol upgrades or XDR schema changes (e.g. key/field renames) may break parsing. |
