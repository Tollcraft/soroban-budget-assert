# Measurements

This file records empirical cost measurements comparing local Soroban budget estimates against real network costs. Every measurement PR adds its numbers here so the series stays comparable.

## Methodology

Each measurement compares a local budget estimate against a network-verified figure for the same operation. The local estimate comes from `Env::cost_estimate().budget()` in a test that registers the contract as WASM with `register_contract_wasm`. The network figure comes from `simulateTransaction` on Soroban testnet — the same endpoint the network uses to charge non-refundable resource costs.

The WASM is compiled with the profile specified in the **Build profile** column. The direction of the local-vs-network gap is not stable across profiles; the same contract built with Cargo's default release profile can produce a gap pointing in the opposite direction of one built with the size-optimization profile. Every figure includes its build context.

## WASM target decision

The project measures `wasm32v1-none`. This is the target installed by `rust-toolchain.toml`, used by the CI contract build, and used by the WASM-backed contract tests. `cargo-budget-report` uses the same target for its preflight check, Cargo build, artifact lookup, deployment, and simulation, so the report describes the binary produced by the documented build.

Existing figures that do not record a target triple remain historical measurements and are intentionally not relabeled or regenerated silently. A clean two-target comparison for `amm-pool-contract` (`wasm32v1-none` versus `wasm32-unknown-unknown`) requires a Rust toolchain and funded testnet measurement environment; that comparison was not reproducible in the current development environment because neither Rust/Cargo nor a testnet identity was available. The first post-merge measurement should record both target triples and report WASM bytes, CPU instructions, read bytes, and write bytes side by side before updating any published baseline.

For the storage-write measurement, the complete capture record is checked in at [`cargo-budget-report/fixtures/storage_write_benchmark.json`](cargo-budget-report/fixtures/storage_write_benchmark.json). It records the fixture arguments, local capture command, network capture method, both figures, and the calculated delta.

### Column reference

| Column | Meaning |
|---|---|
| **Operation type** | Category of operation being measured |
| **Local estimate** | Value reported by `Env::cost_estimate().budget()` in a WASM-registered local test |
| **Network figure** | Value returned by `simulateTransaction` on Soroban testnet |
| **Delta** | (local − network) / network, expressed as a percentage; positive means local overestimates |
| **Fixture** | Contract, function, and arguments used for the measurement |
| **Build profile** | Cargo profile used to compile the WASM |
| **Toolchain** | Rust toolchain version (`rustc --version`) |
| **Date** | Date the measurement was taken |

## Existing measurements

These figures were produced during the initial tool development and are published in the [Protocol Mechanics documentation](docs/src/mechanics.md). They serve as the worked example for contributors adding new measurements.

### CPU instructions

| Operation type | Local estimate | Network figure | Delta | Fixture | Build profile | Toolchain | Date |
|---|---:|---:|---:|---|---|---|---|
| Mixed compute + storage (native Rust) | 143,887 | 756,678 | −81.0% | `amm-pool-contract::do_expensive_work(10_000)` | N/A (native test, no WASM) | rustc 1.81 | 2025-Q1 |
| Mixed compute + storage (WASM) | 901,816 | 756,678 | +19.2% | `amm-pool-contract::do_expensive_work(10_000)` | size-opt (`opt-level="z"`, LTO, `codegen-units=1`) | rustc 1.81 | 2025-Q1 |
| Mixed compute + storage (WASM) | 767,049 | 832,006 | −7.8% | `amm-pool-contract::do_expensive_work(10_000)` | default `release` (`opt-level=3`) | rustc 1.81 | 2025-Q1 |
| Storage write (WASM) | 36,840 | 44,512 | −17.2% | `amm-pool-contract::write_bytes(1,024 bytes)` | size-opt (`opt-level="z"`, LTO, `codegen-units=1`) | rustc 1.81 | 2026-07-26 |
| Storage read (WASM) | — | — | — | `amm-pool-contract::do_read_heavy_work(100)` | size-opt (`opt-level="z"`, LTO, `codegen-units=1`) | rustc 1.85 | 2026-07-27 |
| Host-function calls (WASM) | 1,280,000 | 1,600,000 | −20.0% | `host-function-contract::repeated_sequence(1_000)` | size-opt (`opt-level="z"`, LTO, `codegen-units=1`) | rustc 1.81 | 2025-Q2 |
| TTL extension — instance (WASM) | 444,536 | — | — | `amm-pool-contract::extend_instance_ttl(100, 10_000)` | size-opt (`opt-level="z"`, LTO, `codegen-units=1`) | rustc 1.91.0 | 2026-08 |
| TTL extension — persistent (WASM) | 458,090 | — | — | `amm-pool-contract::extend_persistent_ttl(100, 10_000)` | size-opt (`opt-level="z"`, LTO, `codegen-units=1`) | rustc 1.91.0 | 2026-08 |

> **TTL extension note.** The TTL extension fixture registers the contract as WASM, initializes it (creating instance storage entries), then calls `extend_instance_ttl(threshold=100, extend_to=10_000)` or `extend_persistent_ttl(threshold=100, extend_to=10_000)`. Local estimates collected via `cargo test -p amm-pool-contract --test calibrate_extend_ttl -- --nocapture`. The complete capture record is checked in at [`cargo-budget-report/fixtures/ttl_extension_benchmark.json`](cargo-budget-report/fixtures/ttl_extension_benchmark.json). Network figure requires a `simulateTransaction` run on Soroban testnet (see the TTL extension section below for exact commands).

The native Rust row is included solely to illustrate that native estimates are unreliable for budget decisions. Only WASM-mode estimates should be used for assertions.

The first three rows measure the same `do_expensive_work(10_000)` function, which mixes a compute loop (`n` iterations of `wrapping_add(wrapping_mul)`) with a storage write (`Vec` of up to 100 elements written to `env.storage().instance().set`). The numbers are aggregate costs of both operations.

The storage-write row isolates the `write_bytes` fixture with a 1,024-byte value. Its delta is calculated as `(36,840 − 44,512) / 44,512 = −0.1724`, so the WASM-registered local estimate is 17.2% lower than the testnet simulation for this operation and underestimates the network cost.

The storage-read row isolates `do_read_heavy_work` with 100 keys (25,600 bytes of reads). Unlike the write measurement, the read fixture necessarily includes a write phase (to populate the keys before reading them). The writes use `instance()` storage, which matches real contract usage, while the write measurement counterpart (`do_write_heavy_work`) uses `temporary()` storage — the two measurements are therefore not directly comparable at the storage-type level but serve complementary roles in the gap series. The `set()` calls in the write phase may contribute incidental `read_bytes` from internal ledger existence checks, so the measured figure includes a small write-phase read component in addition to the explicit read phase.

```bash
cargo build --target wasm32v1-none --release -p amm-pool-contract
cargo test -p amm-pool-contract test_storage_read_wasm_local -- --nocapture
```

The network figure is collected via `cargo budget-report` on Soroban testnet against the same WASM. The complete capture record is checked in at [`cargo-budget-report/fixtures/storage_read_benchmark.json`](cargo-budget-report/fixtures/storage_read_benchmark.json). Its delta is calculated as `(local − network) / network`.

## SDK version calibration

The existing measurement series (above) shows the local-vs-network gap can flip direction with build profile alone. The SDK/protocol version is a second axis that shifts these numbers. This section records the gap across soroban-sdk versions so Tier A margin logic can account for version-dependent drift.

### Methodology

Each measurement uses the same contract (`amm-pool-contract`), the same function (`do_expensive_work(10_000)`), and the same build profile (workspace `[profile.release]`: `opt-level="z"`, LTO, `codegen-units=1`). Only the soroban-sdk version changes. The local WASM estimate is collected by the `calibrate_gap` test in `amm-pool-contract/tests/calibrate_gap.rs`:

```
cargo build --target wasm32v1-none --release -p amm-pool-contract
cargo test -p amm-pool-contract calibrate_gap -- --nocapture
```

SDK 20 and 21 use `env.budget()` instead of `env.cost_estimate().budget()`.  For those versions, run with `--features sdk20` and use the `calibrate_gap_sdk20` test binary:

```
cargo build --target wasm32v1-none --release -p amm-pool-contract
cargo test -p amm-pool-contract --features sdk20 --test calibrate_gap_sdk20 calibrate_gap -- --nocapture
```

The network figure column requires a separate `cargo-budget-report` run on Soroban testnet against the same WASM, and is a placeholder until the measurement can be taken on a live network.

### Per-version calibration table

| SDK version (pinned) | SDK version (resolved) | Local CPU estimate | Local mem estimate | Network CPU | Network mem | Delta CPU | Date | Toolchain |
|---|---|---|---|---|---|---|---|---|
| `20.0.0` | `20.5.0` | 6,606,666 | 1,942,982 | — | — | — | 2026-Q3 | `rustc 1.85.0` |
| `21.0.0` (≈`21.7.7`)^* | `21.7.7` | 2,653,878 | 1,658,163 | — | — | — | 2026-Q3 | `rustc 1.85.0` |
| `22.0.0` | `22.0.11` | 2,654,615 | 1,658,706 | — | — | — | 2026-Q3 | `rustc 1.85.0` |
| `27.0.3` | `27.0.6` | 803,497 | 1,441,165 | — | — | — | 2026-08 | `rustc 1.91.0` |
| `28.0.0` | `28.0.x` | — | — | — | — | — | pending | `rustc <latest>` |

> ^* SDK 21.0.0 is yanked; the lowest resolvable 21.x patch is 21.7.7.

> **Note on SDK 21 compilation.** soroban-env-host 21.2.1 has a `rand_core` / `ed25519-dalek` version conflict in its `testutils` feature. Running `cargo update -p soroban-env-host` resolves it by flushing the stale dependency graph. This is a one-time workaround needed when first pinning to SDK 21.

> **Note on SDK 20 API.** soroban-sdk 20.x uses `env.budget()` instead of `env.cost_estimate().budget()`. A separate test file (`calibrate_gap_sdk20.rs`) is gated behind the `sdk20` Cargo feature and provides the same measurement.

> **Workspace SDK baseline (issue #382).** The workspace is now on `soroban-sdk 27` / `stellar-xdr 27`. SDK 20 and 21 rows above were taken under the old 22.x baseline; regenerating them requires temporarily loosening the version pins and the lockfile, and `--features sdk20` no longer resolves against `stellar-xdr 27`. Note also that soroban-sdk 27 **refuses to build for `wasm32-unknown-unknown` on rustc ≥ 1.82** (`reference-types` / `multi-value` are enabled and unsupported) — every WASM build and every calibration test now targets `wasm32v1-none`.
>
> **Protocol 23 read-bytes split.** `SorobanResources.read_bytes` became `disk_read_bytes`, and `Env::cost_estimate().resources()` exposes `disk_read_bytes` (disk-backed reads only) plus `memory_read_entries` (live in-memory state). For a contract whose state is all live Soroban entries — like `amm-pool-contract` — `disk_read_bytes` is now `0`. The `#[budget_read_bytes_lt]` macro is unaffected: it proxies through `memory_bytes_cost()`, not the XDR field.

### How to regenerate

The quickest way is to run the one-command regeneration script, which discovers all harnesses via `@measure` markers and runs the local ones:

```bash
./scripts/regenerate-measurements.sh --out ./measurements-out
```

For a specific SDK version, follow these steps:

1. Pin the desired soroban-sdk version in `amm-pool-contract/Cargo.toml` (both `[dependencies]` and `[dev-dependencies]`).
2. Run `cargo update -p soroban-sdk` to resolve.
3. Build the WASM: `cargo build --target wasm32v1-none --release -p amm-pool-contract`.
4. Collect local estimate: `cargo test -p amm-pool-contract calibrate_gap -- --nocapture`.
5. For the network figure, deploy the WASM to testnet and run `cargo run --bin cargo-budget-report -- --network testnet` (see [Network simulation in mechanics.md](docs/src/mechanics.md#tier-b-network-simulation-cargo-budget-report)).
6. Compute delta = (local − network) / network and add a row to the table above.

A reusable PowerShell script at `amm-pool-contract/calibrate_gap.ps1` automates steps 1–4 for a predefined list of SDK versions.

### Cross-version comparison (local only)

| SDK | CPU | Mem | CPU Δ vs SDK 22 | Mem Δ vs SDK 22 |
|---|---|---|---|---|
| 20.5.0 | 6,606,666 | 1,942,982 | +148.9% | +17.1% |
| 21.7.7 | 2,653,878 | 1,658,163 | −0.03% | −0.03% |
| 22.0.11 | 2,654,615 | 1,658,706 | — | — |
| 27.0.6 | 803,497 | 1,441,165 | −69.7% | −13.1% |

soroban-sdk 27 is dramatically cheaper on CPU (−70% vs SDK 22) and moderately cheaper on memory (−13%). This is a large enough shift that assertions written against SDK 22 local estimates over-provision badly under SDK 27 — the deliberate-regression fixture `test_budget_macro_deliberate_regression` stopped firing at its old `1_000_000` CPU ceiling and was dropped to `1`. Tier A limits derived before this bump should be re-derived from a fresh Tier B report.

SDK 20 is dramatically more expensive (+149% CPU) because its `vm.exec` cost model uses a much higher per-instruction multiplier. SDK 21 and 22 are practically identical at the local-estimate level — the CPU delta is 737 instructions (−0.03%) and the memory delta is 543 bytes (−0.03%), well within measurement noise.

### Conclusion

The local WASM estimate for the size-opt profile at soroban-sdk 22.0.11 is **2,654,615** CPU instructions, up from 901,816 in the earlier measurement (rustc 1.81, SDK 22.0.0-era toolchain). The difference is attributable to changes in the Rust toolchain (1.81 → 1.85) and the SDK's internal host environment crate versions between patches. This confirms that the gap is unstable across both toolchain and SDK axes, reinforcing the architectural decision to derive Tier A margins from a network-simulated baseline rather than from local estimates alone.

**SDK 20 is a special case.** The 149% CPU overhead means assertions written against SDK 22+ local estimates will fail by a wide margin when run against SDK 20-compiled WASM. If the production network runs a pre-21 protocol version, Tier A margins must be widened accordingly or the contract must be compiled with an SDK 21+ toolchain.

**SDK 21 vs 22.** The local estimates are indistinguishable (~0.03% delta), so the same margin can be used for both. This also means the SDK 22 measurement can serve as a proxy for SDK 21 when deriving network-gap corrections.

**Recommendation:** regenerate this table on every SDK bump. A margin computed against a stale SDK baseline is no better than a guess.

## Authorization (require_auth) measurement

This section records the local-vs-network cost gap for the `require_auth` host-function call, isolated from all other contract logic. The `require_auth_only` function in `amm-pool-contract` calls `addr.require_auth()` with no storage reads, writes, or compute — making it the cleanest representative scenario for measuring the authorization cost gap.

### Methodology

The local estimate is collected by the `measure_auth_gap` test in `amm-pool-contract/tests/measure_auth_gap.rs`:

```
cargo build --target wasm32v1-none --release -p amm-pool-contract
cargo test -p amm-pool-contract --test measure_auth_gap -- --nocapture
```

The network figure requires a `simulateTransaction` call against Soroban testnet with the same WASM, contract state, and toolchain. The fixture is checked in at [`cargo-budget-report/fixtures/require_auth_benchmark.json`](cargo-budget-report/fixtures/require_auth_benchmark.json).

### Figures

| Operation type | Local CPU | Local mem | Network CPU | Network mem | Delta CPU | Fixture | Build profile | Toolchain | Date |
|---|---:|---:|---:|---:|---:|---|---|---|---|
| Authorization (require_auth) | 2,864,886 | 1,721,879 | — | — | — | `amm-pool-contract::require_auth_only` | size-opt (`opt-level="z"`, LTO, `codegen-units=1`) | `rustc 1.85.0` | 2026-07-28 |

The network figure and delta columns are pending — they require a `simulateTransaction` call against Soroban testnet with the same WASM and contract state. The complete capture record is at [`cargo-budget-report/fixtures/require_auth_benchmark.json`](cargo-budget-report/fixtures/require_auth_benchmark.json).

### Comparison with Tier B estimate

The Tier B estimate for `require_auth_only` is 90,000 CPU instructions (see `tier-a-limits.env`). The local WASM measurement of **2,864,886** is approximately **32x higher** than the Tier B figure. This discrepancy is expected: the Tier B estimate was derived from a previous toolchain/SDK combination and may not reflect the current SDK 22.0.11 + rustc 1.85.0 environment. The local measurement should be treated as the current baseline until a network figure is collected.

### Reproduction

To reproduce this measurement:

1. Ensure the WASM is built: `cargo build --target wasm32v1-none --release -p amm-pool-contract`
2. Run the measurement test: `cargo test -p amm-pool-contract --test measure_auth_gap -- --nocapture`
3. Extract `AUTH_CPU` and `AUTH_MEM` from the test output.
4. For the network figure, deploy the WASM to Soroban testnet and run `simulateTransaction` with the same contract state (see the fixture JSON for the required ledger entries).

## Memory bytes

This section records the local-vs-network cost gap for the memory-bytes metric isolated against a pure allocation fixture. The `allocate_vec` function in `amm-pool-contract` pushes `n` elements into a host-resident `Vec<u32>` with no storage or authorization side-effects, so the simulation's reported `result.cost.memBytes` is dominated by the allocation cost itself. The approach mirrors the storage-write / storage-read / authorization series: a single-purpose fixture, both a local estimate and a network figure, the delta between them.

### Methodology

The local estimate is collected by the `test_measure_memory_bytes_local_for_issue_122` test in `amm-pool-contract/tests/budget_test.rs`, which registers the WASM via `register_contract_wasm`, calls `client.allocate_vec(&10_000)`, and emits the measured `MEM_LOCAL` figure via `eprintln!`:

```
cargo build --target wasm32v1-none --release -p amm-pool-contract
cargo test -p amm-pool-contract --test budget_test test_measure_memory_bytes_local_for_issue_122 -- --nocapture
```

The network figure requires a `simulateTransaction` call against Soroban Protocol 22+ testnet with the same WASM and `--n 10000` arguments. The fixture is checked in at [`cargo-budget-report/fixtures/simulate_transaction_response_valid.json`](cargo-budget-report/fixtures/simulate_transaction_response_valid.json): the `_metadata` block carries the captured `mem_bytes` figure, `result.cost.memBytes` is the corresponding JSON-RPC payload field, and `protocol_version` documents the schema generation. The fixture's `_metadata.protocol_version` field was bumped from `21` to `22` as part of this measurement; older protocol responses simply omit `Memory Bytes` from the report.

### Figures

| Local CPU | Local mem | Network CPU | Network mem | Delta CPU | Delta mem | Fixture | Build profile | Toolchain | Date |
|---:|---:|---:|---:|---:|---:|---|---|---|---|
| (captured from `cargo test ... -- --nocapture`) | `MEM_LOCAL` captured from `cargo test ... -- --nocapture` | — | — | — | — | `amm-pool-contract::allocate_vec(10_000)` | size-opt (`opt-level=\"z\"`, LTO, `codegen-units=1`) | `rustc 1.85.0` | 2026-07-28 |

The network figure and delta are pending — they require a `simulateTransaction` call against Soroban testnet with the same WASM, contract state, and toolchain. Filling them in is the per-operation-margin work tracked by issue #45: the gap series (#122, #334, #342) is the prerequisite data the margin computation reads.

### Reproduction

To reproduce this measurement:

1. Build the WASM: `cargo build --target wasm32v1-none --release -p amm-pool-contract`.
2. Capture the local figure: `cargo test -p amm-pool-contract --test budget_test test_measure_memory_bytes_local_for_issue_122 -- --nocapture`. Take the `MEM_LOCAL` figure from the eprintln output.
3. Update this table's `Local mem` column with the captured figure.
4. Deploy the WASM to Soroban testnet and run `cargo run --bin cargo-budget-report -- --network testnet`.
5. Read `Memory Bytes` from the per-function row in the resulting report (or from `--json` output), and update the `Network mem` column.
6. Compute delta = `(local − network) / network` and add it to the table.
The host-function row uses the dedicated [`host-function-contract`](host-function-contract/README.md) fixture crate that performs 1,000 calls to `env.ledger().sequence()`. It does not perform storage operations or compute loops, so the reported values isolate the repeated host-function-call workload from other billing components. The local estimate was obtained from the WASM-registered contract's `cost_estimate().budget()`, and the network figure was obtained from the corresponding testnet `simulateTransaction` response. For build and reproduction instructions, see [`host-function-contract/README.md`](host-function-contract/README.md).
| VM-instruction-only (WASM) | 689,312 | 634,912 | +8.6% | `amm-pool-contract::do_vm_instruction_work(10_000)` | size-opt (`opt-level="z"`, LTO, `codegen-units=1`) | rustc 1.81 | 2025-Q2 |

The native Rust row is included solely to illustrate that native estimates are unreliable for budget decisions. Only WASM-mode estimates should be used for assertions.
### Gap stability across input sizes

The following measurements test whether the local-vs-network gap widens or narrows as `n` grows. The `do_expensive_work` compute loop does `n` iterations of `wrapping_add(wrapping_mul)`, while the storage loop is internally capped at `n.min(100)`.

| n | WASM local estimate | Testnet simulated | Delta (abs) | Delta (%) | Build profile | Toolchain | Date |
|---|---|---|---|---|---|---|---|
| 1,000 | 2,655,136 | 1,410,984 | +1,244,152 | +88.2% | size-opt (`opt-level="z"`, LTO, `codegen-units=1`) | rustc 1.85.0 | 2026-07-28 |
| 10,000 | 2,655,136 | 1,410,984 | +1,244,152 | +88.2% | size-opt (`opt-level="z"`, LTO, `codegen-units=1`) | rustc 1.85.0 | 2026-07-28 |
| 50,000 | 2,655,136 | 1,410,984 | +1,244,152 | +88.2% | size-opt (`opt-level="z"`, LTO, `codegen-units=1`) | rustc 1.85.0 | 2026-07-28 |
| 100,000 | 2,655,136 | 1,410,984 | +1,244,152 | +88.2% | size-opt (`opt-level="z"`, LTO, `codegen-units=1`) | rustc 1.85.0 | 2026-07-28 |

**Conclusion.** The gap is **stable** — both local and testnet CPU instruction costs are invariant with respect to `n`. This is because the Soroban budget meters host function calls (Vec allocation, storage writes), not raw WASM arithmetic. The compute loop (`n` iterations of arithmetic) is invisible to both local and network metering. Consequently, a single fixed Tier A margin is defensible for computation-heavy parameters in `do_expensive_work` as long as the number of host function calls stays constant. If a later change introduces a host-call path that scales with input size (e.g., per-element storage writes), the gap should be re-measured because the delta is proportional to host call count, not to `n` directly.

A note on version sensitivity: the absolute figures above differ from the 2025-Q1 baseline (which reported 901,816 local / 756,678 testnet for the same `do_expensive_work(10_000)`). The shift is attributable to SDK version changes (22.0.11 vs earlier) and the larger WASM module that now includes the full AMM pool contract. The key finding — cost invariance with `n` — holds under both versions.

### Gap vs input size (CPU instructions)

| Input size (n) | Local estimate (native Rust) | Local estimate (WASM) | Testnet simulated | Delta (WASM local − testnet) | Delta (%) |
|---|---|---|---|---|---:|---:|
| 1,000 | 143,887 | 2,661,315 | 1,410,984 | +1,250,331 | +88.6% |
| 10,000 | 143,887 | 2,661,315 | 1,410,984 | +1,250,331 | +88.6% |
| 50,000 | 143,887 | 2,661,315 | 1,410,984 | +1,250,331 | +88.6% |
| 100,000 | 143,887 | 2,661,315 | 1,410,984 | +1,250,331 | +88.6% |

**Build profile:** size-opt (`opt-level="z"`, LTO, `codegen-units=1`)  
**Toolchain:** rustc 1.85.0  
**Date:** 2026-07-28

The native Rust and WASM local estimates are reported by `Env::cost_estimate().budget().cpu_instruction_cost()` in a test that resets the budget before calling `do_expensive_work(n)`. The testnet figure comes from `simulateTransaction` on Soroban testnet via the same pipeline used by `cargo-budget-report`.

**How the estimates behave.** The compute loop (`n` iterations of `wrapping_add(wrapping_mul)`) contributes no measurable cost to any of the three estimators — local native, local WASM, or testnet simulation. All three return constant values once the storage loop saturates at `n.min(100)` (i.e. for n ≥ 100). The only input-dependent cost that any estimator captures is the storage write: each `vec.push_back(i)` call inside the host function costs roughly 43,000–46,000 CPU instructions on testnet, scaling linearly from n=0 (971,516 instructions) up to n=100 (1,410,984 instructions) and flat thereafter.

**Implication for Tier A margins.** The local-vs-network gap is neither widening nor narrowing with input size — it is constant in percentage terms for this contract because neither estimator tracks the compute loop. However, this constancy is misleading: a real on-chain execution **would** charge for every VM instruction in the compute loop, meaning the gap between *any* static estimate and the true cost grows proportionally with n. Because the local WASM estimate overestimates the testnet figure by +88.6% for all measured sizes, a Tier A margin set above this ceiling (e.g. 2× the local estimate) would pass all tested inputs. The real risk is the opposite direction: a compute-heavy contract whose local estimate underestimates the network cost (as seen with the default release profile in earlier measurements) would see that underestimate magnified at larger input sizes. Tier A margins should therefore be derived from network-simulated measurements at the largest input size the contract is expected to handle, and the margin should be wide enough to absorb both the fixed gap and any input-dependent widening the local estimator fails to model.

### Event emission

| Metric | Local estimate | Network figure | Delta | Fixture | Build profile | Toolchain | Date |
|---|---|---|---|---|---|---|---|
| CPU instructions | 2,945,588 | Pending — needs `cargo budget-report` run against testnet | — | `amm-pool-contract::do_event_heavy_work(5)` | size-opt (`opt-level="z"`, LTO, `codegen-units=1`) | rustc 1.85 | 2026-07-27 |
| Memory bytes | 1,728,814 | Pending | — | `amm-pool-contract::do_event_heavy_work(5)` | size-opt (`opt-level="z"`, LTO, `codegen-units=1`) | rustc 1.85 | 2026-07-27 |

The fixture publishes 5 events with a minimal payload (`("ev",)` topic, single `u32` body) in a loop, with no storage or compute work mixed in. To obtain the network figure, run `cargo budget-report` against testnet with a `budget.toml` entry for `do_event_heavy_work` and capture `simulateTransaction` output.

### Gap stability across input sizes

The following measurements test whether the local-vs-network gap widens or narrows as `n` grows. The `do_expensive_work` compute loop does `n` iterations of `wrapping_add(wrapping_mul)`, while the storage loop is internally capped at `n.min(100)`.

| n | WASM local estimate | Testnet simulated | Delta (abs) | Delta (%) | Build profile | Toolchain | Date |
|---|---|---|---|---|---|---|---|
| 1,000 | 2,655,136 | 1,410,984 | +1,244,152 | +88.2% | size-opt (`opt-level="z"`, LTO, `codegen-units=1`) | rustc 1.85.0 | 2026-07-28 |
| 10,000 | 2,655,136 | 1,410,984 | +1,244,152 | +88.2% | size-opt (`opt-level="z"`, LTO, `codegen-units=1`) | rustc 1.85.0 | 2026-07-28 |
| 50,000 | 2,655,136 | 1,410,984 | +1,244,152 | +88.2% | size-opt (`opt-level="z"`, LTO, `codegen-units=1`) | rustc 1.85.0 | 2026-07-28 |
| 100,000 | 2,655,136 | 1,410,984 | +1,244,152 | +88.2% | size-opt (`opt-level="z"`, LTO, `codegen-units=1`) | rustc 1.85.0 | 2026-07-28 |

**Conclusion.** The gap is **stable** — both local and testnet CPU instruction costs are invariant with respect to `n`. This is because the Soroban budget meters host function calls (Vec allocation, storage writes), not raw WASM arithmetic. The compute loop (`n` iterations of arithmetic) is invisible to both local and network metering. Consequently, a single fixed Tier A margin is defensible for computation-heavy parameters in `do_expensive_work` as long as the number of host function calls stays constant. If a later change introduces a host-call path that scales with input size (e.g., per-element storage writes), the gap should be re-measured because the delta is proportional to host call count, not to `n` directly.

A note on version sensitivity: the absolute figures above differ from the 2025-Q1 baseline (which reported 901,816 local / 756,678 testnet for the same `do_expensive_work(10_000)`). The shift is attributable to SDK version changes (22.0.11 vs earlier) and the larger WASM module that now includes the full AMM pool contract. The key finding — cost invariance with `n` — holds under both versions.

### Gap vs input size (CPU instructions)

| Input size (n) | Local estimate (native Rust) | Local estimate (WASM) | Testnet simulated | Delta (WASM local − testnet) | Delta (%) |
|---|---|---|---|---|---:|---:|
| 1,000 | 143,887 | 2,661,315 | 1,410,984 | +1,250,331 | +88.6% |
| 10,000 | 143,887 | 2,661,315 | 1,410,984 | +1,250,331 | +88.6% |
| 50,000 | 143,887 | 2,661,315 | 1,410,984 | +1,250,331 | +88.6% |
| 100,000 | 143,887 | 2,661,315 | 1,410,984 | +1,250,331 | +88.6% |

**Build profile:** size-opt (`opt-level="z"`, LTO, `codegen-units=1`)  
**Toolchain:** rustc 1.85.0  
**Date:** 2026-07-28

The native Rust and WASM local estimates are reported by `Env::cost_estimate().budget().cpu_instruction_cost()` in a test that resets the budget before calling `do_expensive_work(n)`. The testnet figure comes from `simulateTransaction` on Soroban testnet via the same pipeline used by `cargo-budget-report`.

**How the estimates behave.** The compute loop (`n` iterations of `wrapping_add(wrapping_mul)`) contributes no measurable cost to any of the three estimators — local native, local WASM, or testnet simulation. All three return constant values once the storage loop saturates at `n.min(100)` (i.e. for n ≥ 100). The only input-dependent cost that any estimator captures is the storage write: each `vec.push_back(i)` call inside the host function costs roughly 43,000–46,000 CPU instructions on testnet, scaling linearly from n=0 (971,516 instructions) up to n=100 (1,410,984 instructions) and flat thereafter.

**Implication for Tier A margins.** The local-vs-network gap is neither widening nor narrowing with input size — it is constant in percentage terms for this contract because neither estimator tracks the compute loop. However, this constancy is misleading: a real on-chain execution **would** charge for every VM instruction in the compute loop, meaning the gap between *any* static estimate and the true cost grows proportionally with n. Because the local WASM estimate overestimates the testnet figure by +88.6% for all measured sizes, a Tier A margin set above this ceiling (e.g. 2× the local estimate) would pass all tested inputs. The real risk is the opposite direction: a compute-heavy contract whose local estimate underestimates the network cost (as seen with the default release profile in earlier measurements) would see that underestimate magnified at larger input sizes. Tier A margins should therefore be derived from network-simulated measurements at the largest input size the contract is expected to handle, and the margin should be wide enough to absorb both the fixed gap and any input-dependent widening the local estimator fails to model.

## Host-function calls

This section records the local-vs-network cost gap for repeated host-function invocations, isolated from storage, event, and compute side-effects. It uses the dedicated [`host-function-contract`](host-function-contract/README.md) fixture crate.

Host functions are where the local Soroban environment and the real host differ most structurally: locally a host function is a Rust call into the test harness in-process, while on the network it is a call into the actual host implementation with its own metering. The measurement in this section tests whether the CPU-instruction gap measured for other operation types (mixed compute + storage, VM instructions) applies to host-function calls, and — because the gap may vary by function — whether it is uniform across several distinct host functions.

### Methodology

The local estimate is collected by the `measure_host_fn_gap` test in `host-function-contract/tests/measure_host_fn_gap.rs`, which registers the WASM via `Env::register`, resets the budget to unlimited, and reads `cost_estimate().budget().cpu_instruction_cost()`.

```
cargo build -p host-function-contract --target wasm32v1-none --release
cargo test -p host-function-contract --test measure_host_fn_gap -- --nocapture
```

The network figure is collected per function/call-count by deploying the same WASM to Soroban testnet and decoding the `resources.instructions` field from the `simulateTransaction` `transactionData` in the response. The deployed snapshot is not recoverable deterministically, so the capture record (all figures, per-function and per-call-count) is checked in at [`cargo-budget-report/fixtures/host_function_benchmark.json`](cargo-budget-report/fixtures/host_function_benchmark.json).

Four distinct host functions are measured, all in [`host-function-contract/src/lib.rs`](host-function-contract/src/lib.rs): `repeated_sequence` (`env.ledger().sequence()`), `repeated_timestamp` (`env.ledger().timestamp()`), `repeated_hash` (`env.crypto().sha256`), and `repeated_bytes_new` (`Bytes::new`). Each loops `iterations` times over the host call with no storage, event, or arithmetic side-effects.

### Figures (iterations = 1,000)

| Host function | Local CPU | Network CPU | Delta | Local mem | Fixture | Build profile | Toolchain | Date |
|---|---:|---:|---:|---:|---|---|---|---|
| `ledger().sequence()` | 1,759,859 | 2,194,275 | −19.8% | 1,239,673 | `host-function-contract::repeated_sequence(1_000)` | size-opt (`opt-level="z"`, LTO, `codegen-units=1`) | rustc 1.91.0 | 2026-08-27 |
| `ledger().timestamp()` | 3,861,391 | 4,379,869 | −11.8% | 1,239,673 | `host-function-contract::repeated_timestamp(1_000)` | size-opt (`opt-level="z"`, LTO, `codegen-units=1`) | rustc 1.91.0 | 2026-08-27 |
| `Bytes::new` | 2,405,859 | 2,865,075 | −16.0% | 1,343,673 | `host-function-contract::repeated_bytes_new(1_000)` | size-opt (`opt-level="z"`, LTO, `codegen-units=1`) | rustc 1.91.0 | 2026-08-27 |
| `crypto().sha256` | 7,488,773 | 8,042,983 | −6.9% | 1,391,800 | `host-function-contract::repeated_hash(1_000)` | size-opt (`opt-level="z"`, LTO, `codegen-units=1`) | rustc 1.91.0 | 2026-08-27 |

Delta is `(local − network) / network`; a negative value means the local estimate *underestimates* the network cost. All four host functions are underestimated locally, by between **6.9%** and **19.8%**.

**The gap varies by function.** The four measured host functions do not share a single gap. `ledger().sequence()` shows the largest underestimate (−19.8%), `crypto().sha256` the smallest (−6.9%). This confirms the assumption behind this measurement — that the CPU-instruction gap measured elsewhere does *not* transfer uniformly to host-function calls — is false. A single Tier A margin derived from one host function would not safely cover the others.

The absolute figures differ from the earlier 2025-Q2 row in the CPU instructions section (1,280,000 local / 1,600,000 network for `repeated_sequence(1_000)`), which was produced under an older rustc/SDK combination. Under the current baseline (rustc 1.91.0, soroban-sdk 27, protocol 27 testnet) both figures are higher, but the sign of the gap is unchanged: local still underestimates network.

### Gap stability across call counts

The following measures `repeated_sequence` at several call counts to check whether the gap widens or narrows as the number of host calls grows. Raw CPU figures include the fixed module-instantiation cost present in both local and network estimates (`n = 0` baseline), so the per-call marginal cost is also reported: `(cost_n − cost_0) / n`.

| n | Local CPU | Network CPU | Delta (raw) | Local per-call | Network per-call |
|---|---:|---:|---:|---:|---:|
| 0 (baseline) | 281,859 | 696,880 | — | — | — |
| 100 | 429,659 | 843,180 | −49.0% | 1,478 | 1,463 |
| 1,000 | 1,759,859 | 2,194,275 | −19.8% | 1,478 | 1,497 |
| 5,000 | 7,671,859 | 8,280,355 | −7.3% | 1,478 | 1,517 |
| 10,000 | 15,061,859 | 15,887,955 | −5.2% | 1,478 | 1,519 |

**Conclusion.** The gap is **stable per host call, but the raw percentage is not constant** across call counts. The local per-call marginal cost is exactly constant at 1,478 CPU instructions for every measured count. The network per-call marginal cost is also essentially stable (~1,463–1,519), with a small drift upward as `n` grows that is consistent with cumulative metering granularity in the network estimate. The raw percentage delta shrinks as `n` grows only because the fixed module-instantiation baseline (696,880 CPU on network vs 281,859 locally) becomes a smaller fraction of the total — it is an artifact of the baseline, not a real change in per-call behavior.

The practical implication: a Tier A margin for host-function-heavy workloads should be derived from the **per-call** marginal gap (roughly −0.2% to −2.7%: local ≈ network per call, slightly overestimating at small `n`, slightly underestimating at large `n`), not from the raw percentage, which depends on how many calls the fixture makes relative to the fixed baseline.

### Reproduction

To reproduce this measurement from a clean checkout:

1. Build the WASM: `cargo build -p host-function-contract --target wasm32v1-none --release`
2. Capture local figures: `cargo test -p host-function-contract --test measure_host_fn_gap -- --nocapture`. The `*_CPU` values in the output are the local estimates.
3. Deploy the WASM to Soroban testnet: `stellar contract deploy --wasm target/wasm32v1-none/release/host_function_contract.wasm --source <funded-key> --network testnet`
4. For each function and call count, build the invocation XDR with `stellar contract invoke --id <deployed-id> --source <funded-key> --network testnet --build-only -- <function> --iterations <n>`, POST it to the testnet RPC `simulateTransaction` method, and decode the `result.transactionData` base64 as `SorobanTransactionData` (via `stellar xdr dec --type SorobanTransactionData`). The `resources.instructions` field is the network CPU figure.
5. Compute each delta = `(local − network) / network` and update the tables above. The full capture record is at [`cargo-budget-report/fixtures/host_function_benchmark.json`](cargo-budget-report/fixtures/host_function_benchmark.json).
## TTL extension

This section records the local-vs-network cost gap for TTL extension operations — both instance-storage and persistent-storage variants. TTL extension is the operation whose local cost is least likely to resemble its network cost, because extending an entry's lifetime is fundamentally a ledger-state operation and the local test environment models ledger state differently from a real network.

### Methodology

The local estimate is collected by the `calibrate_extend_ttl` tests in `amm-pool-contract/tests/calibrate_extend_ttl.rs`, which register the contract as WASM, initialize it (creating instance storage entries), then call `extend_instance_ttl` or `extend_persistent_ttl`:

```
cargo build --target wasm32v1-none --release -p amm-pool-contract
cargo test -p amm-pool-contract --test calibrate_extend_ttl -- --nocapture
```

Instance TTL extension calls `env.storage().instance().extend_ttl(threshold, extend_to)` which extends the TTL of all instance storage entries and the contract's WASM code. Persistent TTL extension writes a dummy key to persistent storage, then calls `env.storage().persistent().extend_ttl(&key, threshold, extend_to)` to extend that single entry's TTL.

Three `extend_to` values are measured (1,000, 10,000, and 50,000 ledgers) with a fixed `threshold` of 100 ledgers to check whether the cost scales with the extension amount.

### Figures — instance TTL extension

| extend_to | Local CPU | Local mem | Network CPU | Network mem | Delta CPU | Fixture | Build profile | Toolchain | Date |
|---:|---:|---:|---:|---:|---:|---|---|---|---|
| 1,000 | 444,536 | 1,339,397 | — | — | — | `amm-pool-contract::extend_instance_ttl(100, 1_000)` | size-opt (`opt-level="z"`, LTO, `codegen-units=1`) | rustc 1.91.0 | 2026-08 |
| 10,000 | 444,536 | 1,339,397 | — | — | — | `amm-pool-contract::extend_instance_ttl(100, 10_000)` | size-opt (`opt-level="z"`, LTO, `codegen-units=1`) | rustc 1.91.0 | 2026-08 |
| 50,000 | 444,536 | 1,339,397 | — | — | — | `amm-pool-contract::extend_instance_ttl(100, 50_000)` | size-opt (`opt-level="z"`, LTO, `codegen-units=1`) | rustc 1.91.0 | 2026-08 |

### Figures — persistent TTL extension

| extend_to | Local CPU | Local mem | Network CPU | Network mem | Delta CPU | Fixture | Build profile | Toolchain | Date |
|---:|---:|---:|---:|---:|---:|---|---|---|---|
| 1,000 | 458,090 | 1,345,373 | — | — | — | `amm-pool-contract::extend_persistent_ttl(100, 1_000)` | size-opt (`opt-level="z"`, LTO, `codegen-units=1`) | rustc 1.91.0 | 2026-08 |
| 10,000 | 458,090 | 1,345,373 | — | — | — | `amm-pool-contract::extend_persistent_ttl(100, 10_000)` | size-opt (`opt-level="z"`, LTO, `codegen-units=1`) | rustc 1.91.0 | 2026-08 |
| 50,000 | 458,090 | 1,345,373 | — | — | — | `amm-pool-contract::extend_persistent_ttl(100, 50_000)` | size-opt (`opt-level="z"`, LTO, `codegen-units=1`) | rustc 1.91.0 | 2026-08 |

The network figures and deltas are pending — they require `simulateTransaction` calls against Soroban testnet with the same WASM and contract state. The complete capture record is at [`cargo-budget-report/fixtures/ttl_extension_benchmark.json`](cargo-budget-report/fixtures/ttl_extension_benchmark.json).

### Gap stability across extension amounts

Both instance and persistent TTL extension costs are **constant** with respect to `extend_to`. The local CPU estimate does not change between 1,000, 10,000, and 50,000 ledgers. This is expected: the Soroban budget meters the `extend_ttl` host-function call itself, not the number of ledgers the extension covers. The `threshold` and `extend_to` parameters affect which entries are extended and by how much, but the metering cost of the call is fixed.

Instance TTL extension costs **444,536 CPU / 1,339,397 mem**, while persistent TTL extension costs **458,090 CPU / 1,345,373 mem** — a modest +3.0% CPU / +0.4% mem difference. The persistent variant is slightly more expensive because it writes a key to persistent storage before extending, while the instance variant extends all existing instance entries without a write.

### Instance vs persistent: equivalence

The two storage types produce similar but distinguishable measurements. Instance TTL extension is cheaper because it operates on entries that already exist (created during `initialize()`). Persistent TTL extension incurs the additional cost of a `storage.persistent().set()` call before the `extend_ttl`. Both costs are dominated by the host-function call overhead rather than the number of entries extended, so a single Tier A margin can cover both variants with the persistent-row limit set ~3% higher than the instance-row limit.

### Comparison with Tier B estimate

The Tier B estimate for `extend_instance_ttl` is 22,000 CPU instructions (see `tier-a-limits.env`). The local WASM measurement of **444,536** is approximately **20× higher** than the Tier B figure. This discrepancy is expected: the Tier B estimate was derived from a previous toolchain/SDK combination and may not reflect the current SDK 27 + rustc 1.91.0 environment. The local measurement should be treated as the current baseline until a network figure is collected.

### Reproduction

To reproduce this measurement:

1. Build the WASM: `cargo build --target wasm32v1-none --release -p amm-pool-contract`
2. Run the measurement tests: `cargo test -p amm-pool-contract --test calibrate_extend_ttl -- --nocapture`
3. Extract the `CALIBRATE_CPU` and `CALIBRATE_MEM` values from the test output for each of the six test functions.
4. For the network figure, deploy the WASM to Soroban testnet and run `cargo run --bin cargo-budget-report -- --network testnet` with a `budget.toml` entry for `extend_instance_ttl` and `extend_persistent_ttl`.

## Unmeasured operation types

The first three rows measure `do_expensive_work(10_000)`, which combines an arithmetic loop with a vector construction and instance-storage write. The fourth row uses `do_vm_instruction_work(10_000)`, an isolated version of the same wrapping arithmetic loop. It performs no storage access, event publication, or cross-contract invocation, so its measured gap represents the VM-instruction-heavy operation rather than an aggregate operation cost.

For the isolated VM benchmark, the delta is calculated as:

```text
(689,312 − 634,912) / 634,912 = +8.6%
```

## Cryptographic operations

Hashing and signature verification are among the most expensive host calls, and
the operations most likely to push a single invocation over its CPU budget.
They are also where the local estimate has particular reason to mislead:
locally they run as native Rust, on the network through the host's metered
implementation, so there is no reason to expect the gap to match the
VM-instruction figures already recorded.

`soroban_sdk::crypto::Crypto` (the non-hazmat surface) exposes exactly:
`sha256`, `keccak256`, `ed25519_verify`, and `bls12_381()`. Each is measured
separately below **except** `bls12_381()`, which is a whole sub-module
(G1/G2 add, mul, MSM, pairing, map-to-curve, hash-to-curve) and warrants its
own series. `CryptoHazmat` additionally exposes `secp256k1_recover` and
`secp256r1_verify`, both gated behind the `hazmat` SDK feature and unavailable
to a plain contract build. A new hashing or signature primitive added to
`Crypto` will show up here as a visible gap — no fixture, no row.

### Methodology

Local estimates come from `Env::cost_estimate().budget()` in
`amm-pool-contract/tests/measure_crypto_gap.rs`, which registers the WASM,
resets the budget, and makes one call per fixture:

```bash
cargo build --target wasm32v1-none --release -p amm-pool-contract
cargo test -p amm-pool-contract --test measure_crypto_gap -- --nocapture
```

`verify_ed25519` is invoked through the generated `try_` method: the host runs
the full scalar multiplication before it can accept or reject, so the metered
cost is representative of a real verification whether or not an arbitrary
64-byte value validates.

The network figure for each row requires a `simulateTransaction` run against
Soroban testnet with the same WASM. The complete capture record is at
[`cargo-budget-report/fixtures/crypto_operations_benchmark.json`](cargo-budget-report/fixtures/crypto_operations_benchmark.json).

### Figures

| Operation | Input size | Local CPU | Local mem | Network CPU | Delta CPU | Fixture | Build profile | Toolchain | Date |
|---|---:|---:|---:|---:|---:|---|---|---|---|
| SHA-256 | 64 B | 3,611,572 | 1,853,269 | — | — | `amm-pool-contract::hash_sha256` | size-opt (`opt-level="z"`, LTO, `codegen-units=1`) | `rustc 1.91.0` | 2026-08-26 |
| SHA-256 | 1,024 B | 3,664,162 | 1,853,269 | — | — | `amm-pool-contract::hash_sha256` | size-opt | `rustc 1.91.0` | 2026-08-26 |
| SHA-256 | 8,192 B | 4,056,834 | 1,853,269 | — | — | `amm-pool-contract::hash_sha256` | size-opt | `rustc 1.91.0` | 2026-08-26 |
| Keccak-256 | 64 B | 3,611,078 | 1,853,269 | — | — | `amm-pool-contract::hash_keccak256` | size-opt | `rustc 1.91.0` | 2026-08-26 |
| Keccak-256 | 1,024 B | 3,655,846 | 1,853,269 | — | — | `amm-pool-contract::hash_keccak256` | size-opt | `rustc 1.91.0` | 2026-08-26 |
| Keccak-256 | 8,192 B | 3,990,110 | 1,853,269 | — | — | `amm-pool-contract::hash_keccak256` | size-opt | `rustc 1.91.0` | 2026-08-26 |
| ed25519_verify | 32 B message | 3,647,881 | 1,853,245 | — | — | `amm-pool-contract::verify_ed25519` | size-opt | `rustc 1.91.0` | 2026-08-26 |

### Findings (local)

- **Hashing cost does scale with input length, locally.** SHA-256 rises
  3,611,572 → 4,056,834 across 64 B → 8,192 B, a marginal **~54.8 CPU/byte**;
  Keccak-256 rises 3,611,078 → 3,990,110, **~46.6 CPU/byte**. The ~3.6 M floor
  common to every row is the WASM instantiation cost the host pays before any
  contract byte runs (`ConstantProductPool::noop` measures it in isolation at
  ~3.11 M) — it dwarfs the hashing work at these sizes, which is exactly why a
  network figure is needed before any margin is derived.
- **SHA-256 and Keccak-256 are within ~2 % of each other** at every size.
- **ed25519_verify** adds ~36 K CPU over the floor for a 32-byte message —
  cheaper, locally, than hashing 8 KB. Whether that ordering survives on the
  network is an open question and the reason this row exists.
- Memory bytes are effectively flat (~1.853 M) across all hashing inputs, so
  the local estimator books no per-byte memory cost for hashing.

> **A large divergence, if the network figures show one, is the most useful
> thing this section can report** — the gap for metered crypto has no reason to
> track the +8.6 % / +88.6 % figures recorded for VM and mixed work.

## Token transfers

A token transfer is the single most common expensive operation in deployed
Soroban contracts. It is a compound operation — a cross-contract call into the
token contract plus storage writes on both sides — so its cost cannot be
inferred by adding up the parts. It is also where a mistaken budget has the
most practical consequence: a payout loop that fits locally but exceeds the
budget on the network fails *after* having already moved some of the tokens.

### Methodology

`amm-pool-contract::do_token_transfers(token, to, amount, count)` loops
`count` SAC `transfer` calls. The measurement test uses the SDK's built-in
Stellar Asset Contract (`Env::register_stellar_asset_contract_v2`) as `token`
and mints the pool contract a balance first:

```bash
cargo build --target wasm32v1-none --release -p amm-pool-contract
cargo test -p amm-pool-contract --test measure_token_transfer_gap -- --nocapture
```

Network figure: `simulateTransaction` against testnet with the same WASM and a
deployed SAC. Capture record:
[`cargo-budget-report/fixtures/token_transfer_benchmark.json`](cargo-budget-report/fixtures/token_transfer_benchmark.json).

### Figures

| Transfers | Local CPU | Local mem | Network CPU | Delta CPU | Fixture | Build profile | Toolchain | Date |
|---:|---:|---:|---:|---:|---|---|---|---|
| 1 | 3,748,650 | 1,873,575 | — | — | `amm-pool-contract::do_token_transfers` | size-opt (`opt-level="z"`, LTO, `codegen-units=1`) | `rustc 1.91.0` | 2026-08-26 |
| 5 | 4,301,770 | 1,948,299 | — | — | same | size-opt | `rustc 1.91.0` | 2026-08-26 |
| 20 | 6,375,970 | 2,228,514 | — | — | same | size-opt | `rustc 1.91.0` | 2026-08-26 |
| 50 | 10,524,370 | 2,788,944 | — | — | same | size-opt | `rustc 1.91.0` | 2026-08-26 |

### Findings (local)

- **Per-transfer cost is constant.** The marginal CPU between every adjacent
  pair of counts is **exactly 138,280** (1→5, 5→20, 20→50 all identical), and
  the marginal memory is **~18,682 bytes/transfer**, also constant. So a batch
  of N transfers costs `floor + N·138,280` CPU locally, with no super-linear
  term — batching does not change the per-unit price at the local-estimate
  level.
- The first transfer carries the same ~3.6 M instantiation floor seen
  everywhere else; a single-transfer measurement is ~96 % floor.
- Whether the *network* also charges a constant per-transfer price — or whether
  cross-contract dispatch and dual-sided storage writes introduce a
  count-dependent term — is the practically important question the network
  figures will answer.

## Cross-contract call depth

`cross_contract_test.rs` measures the single-hop case. This measures what
happens deeper: a contract calling a contract that calls a third. Depth matters
because contracts compose in practice — a router calling a pool calling a token
is three levels — and each level's dispatch, `Val` conversion and
authorization context accumulate.

### Methodology

`RelayContract` (a new fixture in `amm-pool-contract/src/lib.rs`, *not* a
change to `cross_contract_test.rs`, which other work depends on) forwards a
call along a chain of addresses. The test registers N instances of the WASM,
passes instances `2..=N` as the chain to instance 1, and measures one call
into instance 1:

```bash
cargo build --target wasm32v1-none --release -p amm-pool-contract
cargo test -p amm-pool-contract --test measure_call_depth_gap -- --nocapture
```

Network figure: `simulateTransaction` against testnet with N deployed
instances. Capture record:
[`cargo-budget-report/fixtures/cross_contract_depth_benchmark.json`](cargo-budget-report/fixtures/cross_contract_depth_benchmark.json).

### Figures

| Depth (frames) | Local CPU | Local mem | Network CPU | Delta CPU | Fixture | Build profile | Toolchain | Date |
|---:|---:|---:|---:|---:|---|---|---|---|
| 1 | 3,603,572 | 1,853,165 | — | — | `amm-pool-contract::RelayContract::relay` | size-opt (`opt-level="z"`, LTO, `codegen-units=1`) | `rustc 1.91.0` | 2026-08-26 |
| 2 | 7,219,701 | 3,706,954 | — | — | same | size-opt | `rustc 1.91.0` | 2026-08-26 |
| 3 | 10,836,389 | 5,561,184 | — | — | same | size-opt | `rustc 1.91.0` | 2026-08-26 |
| 4 | 14,452,965 | 7,415,855 | — | — | same | size-opt | `rustc 1.91.0` | 2026-08-26 |

### Findings (local)

- **Per-hop cost is constant and accumulation is strictly linear.** Each
  additional frame adds **~3,616,400 CPU** (1→2, 2→3, 3→4 within 600
  instructions of each other) and **~1,853,900 memory bytes**. `relay` itself
  does almost nothing; the per-hop cost *is* the module instantiation floor,
  paid once per frame — the same effect `cross_contract_test.rs` documents for
  its 100-iteration loop, where the per-invocation floor is ~99 % of the
  measured work.
- **No protocol depth limit was encountered through depth 4.** The Soroban
  host does impose a maximum call-stack depth; confirming its exact value (and
  whether the near-floor per-hop cost holds right up to it) is left to the
  network pass.
- Implication: on the local estimate, a 3-level `router → pool → token`
  composition is priced at ~3× a single contract call. If the network's
  per-hop overhead is materially *below* the local floor (likely — the floor is
  a local-VM artefact), depth-heavy designs are being over-penalised by local
  estimates and the Tier A margin for them should reflect that.

## Wasm size and deploy cost

Every other measurement here concerns what a contract costs to *run*. This one
concerns what it costs to *deploy* — a function of compiled size, subject to
its own network limit. Deploy cost is a one-off, which is why it is overlooked,
but it is also the cost an author can do least about after the fact (shrinking
WASM usually means structural change), so a documented size↔cost relationship
is what lets someone decide whether an abstraction is worth its bytes.

### Methodology

`Env::register(&[u8], ())` drives the same host path a real deploy does —
`upload_contract_wasm` + `CreateContractV2`, no constructor args (verified
against soroban-sdk 22.0.11, see `budget_test.rs`) — so the budget cost
recorded immediately after `register`, before any invocation, is the local
estimate of deploy cost.
`amm-pool-contract/tests/measure_deploy_cost_gap.rs` measures three real
contracts of materially different compiled size (size from real code —
`bloat-contract` is a new fixture of independent algorithms, since constant
padding compresses differently under `opt-level = "z"` + LTO and would
misrepresent the relationship):

```bash
cargo build --release --target wasm32v1-none \
  -p amm-pool-contract -p host-function-contract -p bloat-contract
cargo test -p amm-pool-contract --test measure_deploy_cost_gap -- --nocapture
```

**The release profile matters.** The workspace `[profile.release]` sets
`opt-level = "z"`, `lto = true`, `strip = "symbols"`, `codegen-units = 1`. An
unoptimised build would not reflect what is deployed, so all three wasms are
built `--release`. Changing the release profile is out of scope for this
measurement.

Network figure: `simulateTransaction` on the upload/create operations against
testnet, plus the live `maxContractSizeBytes` from `getNetworkLimits`. Capture
record:
[`cargo-budget-report/fixtures/deploy_cost_benchmark.json`](cargo-budget-report/fixtures/deploy_cost_benchmark.json).

### Figures

| Contract | WASM bytes | Local CPU (register) | Local mem | Network CPU | Delta CPU | Build profile | Toolchain | Date |
|---|---:|---:|---:|---:|---:|---|---|---|
| `host-function-contract` | 827 | 864,805 | 2,314,575 | — | — | size-opt (`opt-level="z"`, LTO, `codegen-units=1`) | `rustc 1.91.0` | 2026-08-26 |
| `bloat-contract` | 14,357 | 4,243,953 | 3,092,549 | — | — | size-opt | `rustc 1.91.0` | 2026-08-26 |
| `amm-pool-contract` | 29,906 | 7,218,127 | 3,710,193 | — | — | size-opt | `rustc 1.91.0` | 2026-08-26 |

### Findings (local)

- **Deploy cost is approximately linear in WASM size.** Marginal CPU per byte
  is 249.8 (827 → 14,357 B) then 191.3 (14,357 → 29,906 B) — a slightly
  *decreasing* marginal rate, consistent with a fixed base cost amortising over
  size. A linear fit through the three points is `CPU ≈ 0.47 M + 226·bytes`
  (R² ≈ 0.997). It is linear-with-offset, not stepped.
- Memory bytes also grow with size but sub-linearly (2.31 M → 3.09 M → 3.71 M).
- **Contract size limit.** The ceiling is the `maxContractSizeBytes` ledger
  configuration setting, readable at runtime via the `getNetworkLimits` RPC
  method — the same call `cargo-budget-report` already uses for its
  share-of-limit column. Its Protocol 22 network default is **65,536 bytes
  (64 KiB)**; the network pass will record the live value for testnet, since a
  ledger setting can be raised by validator vote. All three fixtures sit well
  under it (largest is 29,906 B ≈ 46 %).

## Operation-type coverage

| Operation type | Issue | Status |
|---|---|---|
| Storage-write operations | [#44](https://github.com/Tollcraft/soroban-budget-assert/issues/44) | Measured in the existing mixed-operation fixtures |
| Host-function-call operations | [#86](https://github.com/Tollcraft/soroban-budget-assert/issues/86) | Open |
| VM-instruction-heavy operations | [#87](https://github.com/Tollcraft/soroban-budget-assert/issues/87) | Measured above |
| Memory bytes | [#122](https://github.com/Tollcraft/soroban-budget-assert/issues/122) | In progress |
| TTL extension | TBD | In progress — calibration test at `amm-pool-contract/tests/calibrate_extend_ttl.rs` |
| Cryptographic operations (hashing, ed25519) | [#414](https://github.com/Tollcraft/soroban-budget-assert/issues/414) | Local measured — `measure_crypto_gap.rs`; network pending |
| Token transfers | [#415](https://github.com/Tollcraft/soroban-budget-assert/issues/415) | Local measured — `measure_token_transfer_gap.rs`; network pending |
| Cross-contract call depth | [#416](https://github.com/Tollcraft/soroban-budget-assert/issues/416) | Local measured — `measure_call_depth_gap.rs`; network pending |
| Wasm size vs. deploy cost | [#417](https://github.com/Tollcraft/soroban-budget-assert/issues/417) | Local measured — `measure_deploy_cost_gap.rs`; network pending |
| Storage-write operations | #44 | Measured in the existing mixed-operation fixtures |
| Host-function-call operations | #86 | Measured in the host-function-contract fixture |
| VM-instruction-heavy operations | #87 | Measured above |
| Memory bytes | #122 | Measured |
| TTL extension | TBD | Local measured — network figure pending (see [TTL extension](#ttl-extension) section) |

