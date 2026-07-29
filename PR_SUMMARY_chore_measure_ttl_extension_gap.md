Closes #136

# feat: add TTL extension budget gap calibration test and fixture

TTL extension writes ledger state and is charged on a different basis to an
ordinary storage write. Contracts that manage entry lifecycle call it on hot
paths, and because archival only bites on a long-running network, local
estimates for it are the least likely to resemble reality.

This PR continues the measurement series (#44 storage writes, #86
host-function calls, #87 VM instructions) by adding a calibration test for
`extend_instance_ttl` that captures the local budget estimate, plus a fixture
JSON to record the network figure when it becomes available.

## What ships in this PR

- **`amm-pool-contract/tests/calibrate_extend_ttl.rs`** — New calibration test that:
  - Registers `amm-pool-contract` as WASM via `register_contract_wasm`
  - Initializes the contract (creating instance storage entries)
  - Calls `extend_instance_ttl(threshold=100, extend_to=10_000)` — matching the
    existing `test_budget_extend_ttl_isolated` test
  - Prints `CALIBRATE_CPU` and `CALIBRATE_MEM` for the local estimate
  - Gated with `#![cfg(not(feature = "sdk20"))]` for SDK 22+ API compatibility
  - Follows the identical pattern established by `calibrate_gap.rs`

- **`cargo-budget-report/fixtures/ttl_extension_benchmark.json`** — New fixture
  JSON documenting:
  - Operation type, contract, function, and arguments
  - Build profile (`size-opt`: `opt-level="z"`, LTO, `codegen-units=1`)
  - Toolchain (`rustc 1.85.0`)
  - Capture commands for both local and network collection
  - Placeholder measurement fields with a note on how to fill them in
  - Follows the format of `storage_write_benchmark.json` (#44)

- **`MEASUREMENTS.md`** — Updated:
  - Added TTL extension row to the CPU instructions table (with placeholder
    dashes until measurements are collected)
  - Added a note block explaining the fixture, registration, and collection
    commands
  - Added "TTL extension" to the Unmeasured operation types table with
    "In progress" status

## Figures

| Metric | Local estimate | Network figure | Delta |
|---|---|---:|---:|
| CPU instructions | *(run `cargo test -p amm-pool-contract --test calibrate_extend_ttl -- --nocapture`)* | *(deploy WASM → `simulateTransaction`)* | — |
| Memory bytes | *(same command)* | *(same command)* | — |

> **To collect:** Build the WASM with
> `cargo build --target wasm32-unknown-unknown --release -p amm-pool-contract`,
> then run the calibration test above. Network figures require a testnet
> `simulateTransaction` run. Update `ttl_extension_benchmark.json` and this
> table with the results.

## How the gap measurement works

1. **Local estimate** — `env.cost_estimate().budget()` in a WASM-registered
   test (same approach as `test_budget_extend_ttl_isolated` but with budget
   reset to unlimited and explicit print output).

2. **Network figure** — `simulateTransaction` on Soroban testnet, the same
   endpoint the network uses to charge non-refundable resource costs.

3. **Delta** — `(local − network) / network`, expressed as a percentage.
   Positive means local overestimates; negative means local underestimates.

## Files changed

| File | Change |
|---|---|
| `amm-pool-contract/tests/calibrate_extend_ttl.rs` | **New.** Calibration test for `extend_instance_ttl` local budget estimates. Gated with `#![cfg(not(feature = "sdk20"))]`. Prints `CALIBRATE_CPU` and `CALIBRATE_MEM`. |
| `cargo-budget-report/fixtures/ttl_extension_benchmark.json` | **New.** Fixture recording operation type, arguments, build profile, toolchain, capture commands, and measurement placeholders. |
| `MEASUREMENTS.md` | Added TTL extension row to CPU instructions table, explanatory note block, and "In progress" entry in the Unmeasured types table. |

## Design notes

- **Consistent pattern.** The test mirrors `calibrate_gap.rs` exactly: WASM
  registration, budget reset, function call, budget print. This keeps the
  measurement series comparable.
- **Arguments match existing tests.** `threshold=100, extend_to=10_000` are the
  same values used in `test_budget_extend_ttl_isolated`, so the calibration
  number is directly comparable to the budget assertion tests.
- **Initialize before extend.** Instance storage entries must exist before
  `extend_ttl` is called; `client.initialize()` is called first, matching
  `setup_wasm()` in `budget_test.rs`.
- **Placeholder values.** The fixture JSON and MEASUREMENTS.md use `null`/`—`
  for measurements that require local execution or testnet access. Commands to
  collect both are documented in the fixture and the PR body.

## Checklist

- [x] Added calibration test (`calibrate_extend_ttl.rs`)
- [x] Added fixture JSON (`ttl_extension_benchmark.json`)
- [x] Updated `MEASUREMENTS.md`
- [ ] Passed `cargo fmt --all -- --check` — deferred (no local toolchain)
- [ ] Passed `cargo clippy --workspace --all-targets -- -D warnings` — deferred
- [ ] Passed `cargo test --workspace` — deferred
- [ ] Collected local estimate (see Figures table above)
- [ ] Collected network figure (requires testnet)
- [x] Code reviewed by code-reviewer-deepseek
- [x] Matches `contributing.md` and existing PR summary format
