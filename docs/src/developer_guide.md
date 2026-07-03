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
| `example-contract` | Reference contract (`do_expensive_work`) plus the integration tests that double as the research measurements. |

`budget.toml` at the root configures the CLI for the example contract, and `.github/workflows/budget.yml` runs the Tier A tests in CI.

## Testing

The macro tests execute the compiled WASM, so build it first:

```bash
cargo build -p example-contract --release --target wasm32-unknown-unknown
cargo test
```

`example-contract/tests/budget_test.rs` contains four tests:

- `test_budget_raw_rust` / `test_budget_wasm` — print raw-Rust vs. WASM local cost estimates (the source of the measured-gap figures in Mechanics).
- `test_budget_macro_gated` — a passing assertion at the 950,000 CPU limit.
- `test_budget_macro_deliberate_regression` — asserts an intentionally low limit (600,000) and expects the macro's panic, proving the gate fires.

To exercise the CLI end-to-end against testnet (requires the funded `alice` identity):

```bash
cargo run -p cargo-budget-report -- budget-report
```

## Extending

- **New assertion metrics** — follow the pattern in `budget-macros/src/lib.rs`: parse the limit literal, append a check on `env.cost_estimate().budget()`, keep the failure message explicit. Add a passing test and a `#[should_panic]` regression test in `example-contract`.
- **CLI changes** — no panics; return `anyhow::Result` with `.context()` on every external call (network, `stellar` invocations, file I/O). Any new output must also work under `--json`.
- **Docs** — this site is mdBook (`docs/`); CI deploys it to GitHub Pages on push to `main`. Add pages to `docs/src/SUMMARY.md`.
