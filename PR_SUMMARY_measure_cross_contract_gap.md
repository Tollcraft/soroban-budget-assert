Closes #137

# chore(measurements): add cross-contract call gap measurement scaffolding

## Summary

A representative cross-contract invocation now has a measurement row, a fixture, and the exact reproducer commands for both the local WASM estimate and the network `simulateTransaction` figure. Both numbers are still pending capture because the prerequisites (rustc 1.85.0 + `wasm32v1-none` locally; `stellar` CLI + friendbot-funded `alice` network-side) are not available in the sandbox this PR was prepared in, but the infrastructure closes #137's structural half: the format is established, the fixture is checked in, and the reproducer is re-runnable by anyone with the prerequisites — meaning #45 (per-Tier-A margins) can finally consume a real cross-contract gap value once the capture commands are executed on a host that has them.

## What ships in this PR

| File | Change |
|---|---|
| `cargo-budget-report/fixtures/cross_contract_benchmark.json` | **New.** Machine-readable record of the measurement: fixture description, build + toolchain context, the two capture commands verbatim, and the four numeric cells of `measurements.cpu_instructions` (`local_estimate`, `network_figure`, `delta`, `delta_percent`) — all four flagged `"TBD"` until captured. Same shape as `storage_write_benchmark.json` so the eventual population diff is a numbers-only diff. |
| `MEASUREMENTS.md` | **Modified.** One new row added under **Existing measurements → CPU instructions** with em-dashes (`—`) in the numeric cells. The "report prominently if sign contradicts" rule from the issue is now a `>` blockquote directly under that row (was previously buried three section levels deep). A new section `## Cross-contract call gap measurement (in progress)` is added between `## Existing measurements` and `## SDK version calibration` to point readers at the fixture and the capture commands. |

## What does NOT ship in this PR (and why)

- **`measurements.cpu_instructions.local_estimate`** — the `rustc 1.85.0` toolchain and `wasm32v1-none` target are not installed in this sandbox; `cargo test -p amm-pool-contract test_cross_contract_wasm -- --exact --nocapture` cannot be run here. The exact command is in the fixture and `MEASUREMENTS.md` for a contributor with those tools.
- **`measurements.cpu_instructions.network_figure`** — `stellar` CLI is not installed here and a friendbot-funded `alice` keypair is not available, so `cargo run --bin cargo-budget-report -- --network testnet --source alice` cannot be run. The deploy command, the `budget.toml` snippet, and the multi-row-pick pattern are spelled out in the fixture.
- **The two CRLF→LF flips on `.github/workflows/deploy-site.yml` and `.github/workflows/docs.yml`** — `.gitattributes` declares `*.yml text eol=lf`, and the workspace continuously reintroduces CRLF on these two paths, so they ping-pong modified ↔ clean regardless of `git checkout --` / `git restore`. Deliberately not staged in this PR; they need their own fix-PR.
- **"Unmeasured operation types" table** — left untouched. The cross-contract row is now in **Existing measurements → CPU instructions** with em-dashes, which already gives honest visibility without overclaiming a completed measurement.

## How to fill in the TBD numbers

### Local (run anywhere with `rustc 1.85.0` + `wasm32v1-none`)

```bash
cargo test -p amm-pool-contract test_cross_contract_wasm -- --exact --nocapture
```

The test prints `=== CROSS-CONTRACT WASM ===` followed by `CPU instructions` and `Memory bytes`. `rust-toolchain.toml` pins both tools, so the command works on first try from any contributor's checkout.

### Network (run on a machine with `stellar` CLI + friendbot-funded `alice`)

```bash
# 1. Deploy the WASM twice on testnet — once for the caller, once for the helper.
#    `do_cross_contract_work` invokes `HelperContract::multiply` by Address, and both
#    exports come from the same cdylib, so on-chain reproduction needs two addresses.
stellar contract deploy \
    --wasm target/wasm32v1-none/release/amm_pool_contract.wasm \
    --source alice --network testnet
# record as caller_address
stellar contract deploy \
    --wasm target/wasm32v1-none/release/amm_pool_contract.wasm \
    --source alice --network testnet
# record as helper_address

# 2. Populate budget.toml so the caller is invoked against the helper.
#    Argument order matches the Rust signature:
#        pub fn do_cross_contract_work(env: Env, other: Address, n: u32) -> u32
#    (`amm-pool-contract/src/lib.rs:247`).
cat >> budget.toml <<'EOF'
[functions.do_cross_contract_work]
args = ["--other", "<helper_address>", "--n", "100"]
EOF

# 3. Build, auto-deploy, simulate every exported function, emit a row per (pkg, fn, metric).
cargo run --bin cargo-budget-report -- --network testnet --source alice
```

The reported table contains rows for every exported function in the WASM (`deposit`, `swap`, `withdraw`, `do_expensive_work`, `do_cross_contract_work`, `multiply`, …); **pick the `do_cross_contract_work` row**. Other rows in the same WASM are not the cross-contract measurement.

## Sign-of-delta reporting rule

When both figures are populated, compute `delta = (local − network) / network` and report it numerically in `measurements.cpu_instructions.delta_percent`. If the sign contradicts

- the storage-write row (delta = −17.2 %, local underestimates), or
- the size-opt mixed-compute-and-storage row (delta = +19.2 %, local overestimates),

call the contradiction out in **prose** directly below the row in `MEASUREMENTS.md` and in the follow-up PR description. Do not smooth it over — this is the issue's standing instruction: *"If the gap points in the opposite direction to earlier measurements, that is reported prominently rather than smoothed over."*

## Tests / validation

The two files were validated as follows:

- The JSON fixture parses cleanly (verified with `python3 -c 'import json; …'`).
- The required top-level keys (`operation_type`, `fixture`, `build_profile`, `toolchain`, `network`, `measurements`, `capture`) are all present and match the storage-write precedent — pass-2 code-reviewer-minimax-m3 confirmed.
- The new row in `MEASUREMENTS.md` lands directly under the storage-write row with consistent column count; the new `>` blockquote sits directly under the row (between the row and the "The native Rust row is included…" sentence), so the sign-of-delta rule is in the visually prominent location — not three section depths deep.

`cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace` could not be run in this sandbox (no rust toolchain). They will run on CI from the upstream PR branch. **No `.rs` files were touched by this PR**, so they should be green on first run.

## Design notes

- **Two-deploy requirement is documented in-place.** Because `do_cross_contract_work` calls `HelperContract::multiply` by `Address`, and both exports come from the same `cdylib`, on-chain reproduction must deploy the WASM twice — once for the caller, once for the helper. The fixture's `capture.network_command` adds the explicit "pick the `do_cross_contract_work` row out of a multi-row report" warning because that is the trap a contributor who reads `cargo-budget-report`'s CLI description alone is most likely to miss.
- **Em-dash as the TBD marker.** `MEASUREMENTS.md` already uses `—` for TBD cells in the SDK version calibration table (all Network CPU / Network mem columns for SDK 20 / 21 / 22 show `—`); the new cross-contract row uses the same convention so a glance at the source-of-truth file shows the measurement's state honestly.
- **JSON-shape parity with `storage_write_benchmark.json`.** Both fixtures have the same seven top-level keys; a contributor who fills in the new row by reading the storage-write precedent is not tripped up by any new field shape. An earlier draft of this PR added extras (`soroban_sdk`, `network.rpc_endpoint`, a `notes[]` array, separate `*_prereqs` sub-fields, two `measurements.*` blocks) — reviewer-minimax-m3 caught the shape drift and the final files match storage-write byte-for-byte at the key-set level.
- **Blockquote placement, not deep heading.** The sign-of-delta rule sits directly under the table row, not three section depths deep inside `## Cross-contract call gap measurement (in progress)` — the issue's rule is about *prominence*, so the rule is in the prominent location and the longer rationale lives in the section.

## Files changed

```
 cargo-budget-report/fixtures/cross_contract_benchmark.json |  +43
 MEASUREMENTS.md                                             |  +83
 2 files changed, 126 insertions(+)
```

## Checklist

- [x] Added fixture file (the JSON itself encodes the measurement record)
- [x] Updated source-of-truth file (`MEASUREMENTS.md`)
- [x] Matched the upstream `pull_request_template.md` / `PR_SUMMARY_*.md` style
- [x] Followed: figures TBD until captured; no smoothing over
- [x] Verified `Closes #137` keyword is present in this body
- [ ] Passed `cargo test` — **deferred to CI** (rust toolchain absent from this sandbox; no `.rs` files touched)
- [ ] Passed `cargo clippy` — **deferred to CI** (rust toolchain absent)
- [x] Formatted with `cargo fmt` — N/A this PR touches only `.md` and `.json` files
