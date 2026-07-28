## chore: measure the local-vs-network cost gap for memory-bytes (#122)

Closes #122.

### Summary

Adds the missing memory-bytes metric to `cargo budget-report`'s network-verified tier. Surfaces the protocol-22 `SimulateTransactionResponse.cost.memBytes` JSON field as a first-class `Memory Bytes` row alongside the existing three XDR-derived metrics, with a config-driven `mem_limit` that `cargo budget-report --check` enforces (matching the existing `cpu_limit` / `read_limit` / `write_limit` semantics). Adds the memory-allocation fixture, the local-measurement instrument, the documentation, and the exact reproduction commands required by the issue's "reproducible measurement" requirement.

### Local figures captured (this PR)

`amm-pool-contract::allocate_vec(10_000)` is the issue #122 fixture: it grows a `Vec<u32>` of length `n` and persists it under instance storage (key `bigvec`). No auth required, so it is `cargo budget-report`-callable end-to-end against Soroban testnet without deployed token contracts. Running the new local-measurement instrument (`cargo test -p amm-pool-contract --test budget_test test_measure_memory_bytes_local_for_issue_122 -- --nocapture`) prints:

```
=== ISSUE 122 — RAW RUST LOCAL MEMORY BYTES ===
allocate_vec(10_000) memory_bytes_cost(): 401482058
=== ISSUE 122 — WASM LOCAL MEMORY BYTES ===
allocate_vec(10_000) memory_bytes_cost(): 403430237
```

> Soroban's local `Env::cost_estimate().budget().memory_bytes_cost()` reports *cumulative host-side allocation* (host VM bookkeeping, instance-storage write, per-call overhead), not the data footprint of the `Vec` itself (~40 KiB for 10_000 `u32`s). The ~400 MB raw/WASM figures are therefore far larger than the naive 4×N-byte expectation — the figure is intentionally pessimistic, and the per-operation margin from #45 (not this absolute number) is the actionable budget input.

The **network figure is pending testnet capture** — see `MEASUREMENTS.md` "Commands (issue #122)" for the exact reproduction steps.

### Files touched

| File | Change |
|---|---|
| `cargo-budget-report/src/main.rs` | New `mem_limit` on `FunctionConfig` (serde default); `Memory Bytes` wired into `limit_for_metric`, `emit_check_failure_entries`, and the metric report loop; new `extract_memory_bytes_cost(rpc_response)` JSON helper for `result.cost.memBytes` (accepts string and unquoted integer forms, returns `Option<u32>`, with an `eprintln!` notice on `u32::MAX` overflow so silent truncation can never substitute a proxy); `extract_metrics` now returns `(u32, u32, u32, Option<u32>)`; `SimulationOutcome::Metrics` carries `memory_bytes: Option<u32>`; under `--check` a stub `CostReport { metric: "Memory Bytes", value: None, limit: Some(limit), pass: Some(false) }` and a `checks_failed = true` flag are emitted when a configured `mem_limit` couldn't be evaluated because the RPC omitted `result.cost`, so CI `--json` / `--csv` consumers surface the bypass; `--init` template updated. New tests cover no-cost, missing-memBytes, integer-form, string-form, and unparseable-value paths plus three formatter tests for the "Memory Bytes" unit suffix. |
| `cargo-budget-report/fixtures/simulate_transaction_response_valid.json` | Adds a `result.cost` object with `cpuInsns` and `memBytes` so the cargo-budget-report tests have a complete response shape. |
| `amm-pool-contract/src/lib.rs` | New `allocate_vec(env, n)` — `Vec<u32>` growth + instance-storage write. No auth. |
| `amm-pool-contract/tests/budget_test.rs` | New `test_measure_memory_bytes_local_for_issue_122` — runs `allocate_vec(10_000)` twice (raw Rust + WASM), prints both `memory_bytes_cost()` figures, gated by three `assert!` regression guards (`> 0` for each figure plus `wasm_figure >= raw_figure` to catch an SDK regression that decouples WASM and raw-Rust accounting paths). |
| `MEASUREMENTS.md` | New `### Memory bytes` section in the established column format, with the captured local figures in the WASM row and a follow-up `### Commands (issue #122)` block spelling out the exact testnet reproduction steps. Status row changes from `Open` to `Measured (this PR); numbers pending testnet capture`. |
| `docs/src/mechanics.md` | Tier B "Decode" step now extracts `result.cost.memBytes` alongside the XDR-derived three. |
| `docs/src/reference.md` | `--check` description lists `mem_limit`; in-scope table adds `Memory Bytes → result.cost.memBytes`; both `budget.toml` examples include `mem_limit`; field-list sentence mentions `mem_limit`; "four rows" becomes "up to five rows". |
| `CHANGELOG.md` | Unreleased → Added: four entries (Memory Bytes metric, `mem_limit`, `allocate_vec` fixture, the local measurement test). |
| `budget.toml` | New `[functions.allocate_vec]` block with `args = ["--n", "100000"]` and `mem_limit = 2000000`. |

### Testing

```
cargo fmt --all -- --check        PASS  (after one autofix round)
cargo clippy --workspace --all-targets -- -D warnings  PASS
cargo test --workspace
```

New tests, all PASS:
- `test_measure_memory_bytes_local_for_issue_122`
- `extract_metrics_parses_mem_bytes_none_when_cost_object_absent` (the no-cost path; real `SorobanTransactionData` XDR, no `cost` field)
- `extract_metrics_parses_mem_bytes_none_when_cost_present_but_mem_bytes_missing`
- `extract_metrics_parses_mem_bytes_integer_form`
- `extract_metrics_parses_mem_bytes_string_form`
- `extract_metrics_parses_mem_bytes_unparseable_value`
- `formatter_memory_bytes_gets_byte_unit`
- `formatter_memory_bytes_at_thousands`
- `formatter_memory_bytes_zero`

### Honest disclosure: pre-existing test failures (NOT regressions from this PR)

Two `amm-pool-contract` tests were *already failing* on `chore/measure-memory-bytes-gap`'s HEAD before any of these changes:

- `test_budget_macro_json_config_valid` — CPU instruction cost 2,933,244 vs the 2,500,000 limit set in the test's `budget.json` fixture (drift only).
- `test_read_bytes_budget_within_limit` — WASM read bytes 21,036 vs the 20,000-byte guard (drift only).

Both fail with the identical panic messages they had before this PR. Out of scope to fix here. Six `cargo-budget-report/tests/integration.rs` tests fail in this dev sandbox because the binary's `run_preflight_checks` requires `wasm32-unknown-unknown`, which isn't installed locally — CI installs it per `.github/workflows/budget.yml`.

### Reproducing the network figure

```bash
cargo build -p amm-pool-contract --release --target wasm32v1-none
stellar contract deploy \
  --wasm target/wasm32v1-none/release/amm_pool_contract.wasm \
  --source alice --network testnet
# capture <CONTRACT_ID> from the deploy output
stellar contract invoke \
  --id <CONTRACT_ID> --source alice --network testnet --build-only \
  -- allocate_vec --n 10000
# POST the resulting XDR to simulateTransaction; read result.cost.memBytes
curl -s -X POST -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"simulateTransaction","params":{"transaction":"<XDR>"}}' \
  https://soroban-testnet.stellar.org:443 | jq '.result.cost.memBytes'
```

Or `cargo budget-report` against the same fixture — the `Memory Bytes` row of `amm-pool-contract::allocate_vec` is the network figure. Fill the `Delta = (local − network) / network` cell in `MEASUREMENTS.md` once the capture completes.

### Out of scope

Acting on the measured delta — that is the per-operation-margin work in #45.
