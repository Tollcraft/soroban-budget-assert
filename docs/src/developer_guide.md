# Developer Guide

This guide is for developers modifying or extending `soroban-budget-assert` itself.

## Local setup

1. Clone the repository.
2. Install Rust with the WASM target: `rustup target add wasm32-unknown-unknown`.
3. Install the Stellar CLI: `cargo install --locked stellar-cli` (on Debian/Ubuntu, first `sudo apt-get install -y libdbus-1-dev pkg-config libudev-dev`).
4. Create and fund a testnet identity: `stellar keys generate alice --network testnet --fund`.

## Workspace structure

| Crate | Role |
|---|---|
| `budget-macros` | Proc-macro crate. `budget_cpu_lt` / `budget_mem_lt` rewrite a test function's block to append a budget assertion against its `env` variable. |
| `cargo-budget-report` | The CLI (`cargo budget-report` subcommand). Uses `cargo_metadata` for workspace discovery, `wasmparser` for export scanning, shells out to `stellar` for deploy/invoke/XDR decode, and `tabled`/`serde_json` for output. |
| `amm-pool-contract` | Reference contract (`do_expensive_work`) plus the integration tests that double as the research measurements. |

`budget.toml` at the root configures the CLI for the example contract, and `.github/workflows/budget.yml` runs the Tier A tests in CI.

## Testing

The macro tests execute the compiled WASM, so build it first:

```bash
cargo build -p amm-pool-contract --release --target wasm32-unknown-unknown
cargo test
```

`amm-pool-contract/tests/budget_test.rs` contains six tests:

- `test_budget_raw_rust` / `test_budget_wasm` — print raw-Rust vs. WASM local cost estimates (the source of the measured-gap figures in Mechanics).
- `test_budget_macro_gated` — a passing assertion at the 950,000 CPU limit.
- `test_budget_macro_deliberate_regression` — asserts an intentionally low limit (600,000) and expects the macro's panic, proving the gate fires.
- `test_budget_macro_dynamic_env` — asserts a CPU limit read from the `TEST_MAX_CPU` environment variable.
- `test_budget_macro_dynamic_env_fallback` — verifies the fallback behaviour: when the env var is unset, the limit defaults to `u64::MAX` and the assertion passes unconditionally.

To exercise the CLI end-to-end against testnet (requires the funded `alice` identity):

```bash
cargo run -p cargo-budget-report -- budget-report
```

## Extending

- **New assertion metrics** — follow the pattern in `budget-macros/src/lib.rs`: parse the limit literal, append a check on `env.cost_estimate().budget()`, keep the failure message explicit. Add a passing test and a `#[should_panic]` regression test in `amm-pool-contract`.
- **CLI changes** — no panics; return `anyhow::Result` with `.context()` on every external call (network, `stellar` invocations, file I/O). Any new output must also work under `--json`.
- **Docs** — this site is GitBook, synced from the repository via Git Sync (`.gitbook.yaml` points at `docs/src`). Edits merged to `main` publish automatically; no CI step is involved. Add pages to `docs/src/SUMMARY.md` (GitBook's table of contents). GitBook-specific blocks (`{% hint %}`, `{% code title %}`) are available in any page.

## Docs site appearance

The site's look and feel is configured by a space admin in the GitBook app (**space → Customize**), not in this repository. The intended configuration:

- **Theme**: dark mode as the default, with the light/dark toggle enabled.
- **Accent color**: a single vibrant, high-contrast accent (used for links, hint borders, and active nav) against GitBook's deep dark background.
- **Code blocks**: syntax highlighting works from the fence language tags already present in these pages (`rust`, `bash`, `toml`, `json`); enable line numbers for long snippets if desired.

Content and structure changes belong in this repo; theme changes belong in the GitBook UI.

## ⚙️ Supported Versions & Compatibility

* **Supported SDK Version**: `soroban-sdk` = `"22.0.0"` (specifically tested/resolved to `22.0.11` in `Cargo.lock`)
* **Supported XDR Version**: `stellar-xdr` = `"22.1.0"` (used for decoding transaction simulation responses)
* **Corresponding Stellar Protocol**: **Protocol 22**

### Compatibility Matrix

| SDK Version | Protocol Version | Status | Notes |
| :--- | :--- | :--- | :--- |
| **`< 22.0.0`** | `< 22` | **Untested** | Older protocols may use different transaction/resource schemas. |
| **`22.0.x`** | `22` | **Supported** | Matches pinned manifest dependencies (`soroban-sdk` `22.0.0`, `stellar-xdr` `22.1.0`). |
| **`>= 23.0.0`** | `>= 23` | **Untested** | Future protocol upgrades or XDR schema changes (e.g. key/field renames) may break parsing. |
