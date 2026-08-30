# Cost Terms Glossary

This glossary maps the cost terms used across this project to their definitions in Stellar's documentation, the XDR field names in `SorobanTransactionData`, and the display names in `cargo budget-report` output. Each entry notes whether the tool measures, does not measure, or derives the term.

The first block defines the vocabulary this project itself coined — tiers, margins, scenarios, baselines, and the configuration file the tooling is built around. Entries labelled **Origin** are project terms; all later entries come from Stellar's documentation.

---

## Tier A

- **Origin**: coined by this project
- **Where you'll see it**: `#[budget_cpu_lt(N)]` / `#[budget_mem_lt(N)]`, `tier-a-limits.env`, CI workflow step names
- **What it is**: The local fast-fail tier. The `budget-macros` attribute macros rewrite a test's body so a strict (`<`) assertion on the local cost estimate runs on every path out of the test, and any breach fails `cargo test`. It needs no network and is deterministic, so it is safe to run on every push and pull request.

**Docs**: [Protocol Mechanics](mechanics.md#tier-a-local-fast-fail-budget-macros); limits can be generated into `tier-a-limits.env` rather than hand-written — see [Deriving Limits](deriving_limits.md).

---

## Tier B

- **Origin**: coined by this project
- **Where you'll see it**: every `cargo budget-report` invocation and its `--json` output
- **What it is**: The network-simulation tier. The CLI builds each contract's WASM, deploys it to the configured network, and reports what `simulateTransaction` says each exported function really costs. These figures are ground truth — the input Tier A limits are derived from — though they vary slightly with ledger state.

**Docs**: [Protocol Mechanics](mechanics.md#tier-b-network-simulation-cargo-budget-report)

---

## Margin

- **Origin**: coined by this project
- **Where you'll see it**: `--margin-{cpu,memory,read,write}` flags, the `[margin]` block of `budget.toml`, `tier-a-limits.provenance.md`
- **What it is**: A per-metric multiplier applied to Tier B values when deriving Tier A limits: `tier_a_limit = ceil(tier_b_value × margin)`. There are four margins (CPU, memory, read bytes, write bytes), supplied via `--margin-*` flags or the `[margin]` block of `budget.toml`; none has a default, because the project treats the margin as data that must be stated explicitly so every derived limit stays auditable. Too small a margin produces limits that fail on measurement noise; too large one masks real regressions.

**Docs**: [Deriving Limits](deriving_limits.md); rationale in `cargo-budget-report/src/derive.rs` (`Margin`) and the `--margin-cpu` doc comment in `cargo-budget-report/src/main.rs`.

---

## Scenario

- **Origin**: coined by this project
- **Where you'll see it**: `[[scenarios.<name>]]` blocks in `budget.toml`, derived env-var keys like `TIER_A__AMM_POOL_CONTRACT__SCENARIO__FULL_WORKFLOW__CPU`
- **What it is**: A named multi-step workflow declared as a list of component functions in `budget.toml`. During limit derivation the components' Tier B values are summed into a single Tier A `KEY=VALUE` per metric, so one test exercising the whole workflow (e.g. `deposit` → `swap` → `withdraw`) asserts against one limit instead of several.

**Docs**: [Deriving Limits](deriving_limits.md)

---

## Baseline

- **Origin**: coined by this project
- **Where you'll see it**: `--record-baseline` / `--check-baseline`, `budget-baseline.toml`; and the `baseline = …` argument on the budget macros
- **What it is**: Two related but distinct things in this project:
  - **Regression snapshot** — a TOML file of network-simulated costs per function, recorded with `cargo budget-report --record-baseline` and enforced on later runs by `--check-baseline` within a configured tolerance; a metric exceeding its allowed budget exits non-zero.
  - **Marginal-cost probe** — a measurement of a deliberately minimal call (such as the example contract's `noop`), passed via the macros' `baseline` argument and subtracted from a raw local measurement so only the *marginal* cost is asserted against the limit.
  
  The snapshot pins what the network charged; the probe removes what the local VM charges before your contract logic runs.

**Docs**: [End-User Guide, Step 6](user_guide.md#step-6-optional-catch-regressions-on-the-workspace-with-a-baseline) for the snapshot; the `noop` baseline probe doc comment in `amm-pool-contract/src/lib.rs` for marginal-cost assertions.

---

## Marginal cost

- **Origin**: coined by this project
- **Where you'll see it**: macro assertion messages ("marginal: N measured − M baseline"), the `baseline` argument on `budget_cpu_lt` / `budget_mem_lt` / `budget_read_bytes_lt` / `budget_write_bytes_lt`, and `cpu_baseline` / `mem_baseline` on `budget_lt`
- **What it is**: The cost model where a baseline probe's measurement is subtracted from a raw measurement and the difference — not the raw figure — is compared against the limit. The subtraction saturates at zero, so noise below the probe reports honestly as no measurable marginal cost rather than wrapping to a huge value; the raw measurement is taken before the probe expression is evaluated so the probe cannot perturb it.

**Docs**: [Marginal-cost baseline subtraction](reference.md#marginal-cost-baseline-subtraction) in the Tool Reference for the full mechanics; `generate_metric_assert` in `budget-macros/src/lib.rs` for the implementation; usage in `amm-pool-contract/tests/budget_test.rs`.

---

## Local-vs-network gap

- **Origin**: coined by this project
- **Where you'll see it**: "The measured gap" section of Protocol Mechanics, the measurement series in Measurements
- **What it is**: The difference between a local estimate and the network's simulated cost of the same WASM. Its direction is not stable: it has been observed at double-digit percentages in either direction depending on build profile, and it shifts across `soroban-sdk` versions. This instability is why the tool trusts only network simulation for budget decisions and uses local estimates only for fast-fail gating.

**Docs**: [Protocol Mechanics — The measured gap](mechanics.md#the-measured-gap); per-version calibration in [Measurements](measurements.md#sdk-version-calibration)

---

## budget.toml

- **Origin**: coined by this project
- **Where you'll see it**: workspace root of any project using `cargo budget-report`
- **What it is**: The CLI's configuration file, found by walking upward from the current directory. It holds network and source-account overrides, per-function arguments, limits, and tolerances, plus the `[margin]` and `[[scenarios.<name>]]` blocks consumed by `--derive-limits`. CLI flags take precedence over values set here.

**Docs**: [Tool Reference — Configuration](reference.md#configuration-budgettoml)

---

## CPU Instructions

- **Tool display name**: `CPU Instructions`
- **XDR field**: `resources.instructions` (extracted from `SorobanTransactionData`)
- **What it is**: The count of CPU instruction executions attributed to the transaction during metering. Metering accounts uniformly for both Wasm instructions executed in the guest VM and the calibrated-equivalent cost of host functions. CPU instructions are summed throughout execution and checked against the transaction's declared limit; if the limit is exceeded the transaction fails before completion.
- **Measurement status**: **measures** — `cargo budget-report` decodes this field from `simulateTransaction` output and reports it in every row.

{% hint style="info" %}
Stellar's fees and metering documentation states: _"execution of Wasm instructions is accounted for as a host cost type `WasmInsnExec`, which has a constant CPU cost per Wasm instruction."_ The reported number is the total inclusive of both guest and host metering.
{% endhint %}

**Stellar docs**: [Fees, Resource Limits, and Metering — Metering](https://developers.stellar.org/docs/learn/fundamentals/fees-resource-limits-metering#metering)

---

## Read Bytes (disk-read bytes)

- **Tool display name**: `Read Bytes`
- **XDR field**: `resources.disk_read_bytes` (extracted from `SorobanTransactionData`)
- **Stellar documentation term**: "bytes read from the ledger" / "disk-read bytes"
- **What it is**: The total number of bytes read from ledger entries during the transaction's execution. This includes reading persistent, instance, and temporary storage entries and any contract code or instance entry loaded by the host. "Disk" in the XDR name refers to on-disk ledger storage as distinct from in-memory access, though starting with Protocol 23 ([CAP-0066: Soroban In-Memory Read Resource](https://github.com/stellar/stellar-protocol/blob/master/core/cap-0066.md)), reads are accounted against a separate, cheaper in-memory read resource — but `disk_read_bytes` is the XDR field the RPC returns and the field this tool reports.
- **Measurement status**: **measures** — `cargo budget-report` reads `parsed["resources"]["disk_read_bytes"]` and labels it `"Read Bytes"` in output.

**Stellar docs**: [Fees, Resource Limits, and Metering — Resource fee](https://developers.stellar.org/docs/learn/fundamentals/fees-resource-limits-metering#resource-fee) (refers to "bytes read from the ledger")

---

## Write Bytes

- **Tool display name**: `Write Bytes`
- **XDR field**: `resources.write_bytes` (extracted from `SorobanTransactionData`)
- **Stellar documentation term**: "bytes written to the ledger"
- **What it is**: The total number of bytes written to ledger entries during the transaction. This includes new and modified storage entries. Write bytes is a primary cost driver because ledger writes are charged at a dynamic rate that increases when the global ledger size grows.
- **Measurement status**: **measures** — `cargo budget-report` reads `parsed["resources"]["write_bytes"]` and labels it `"Write Bytes"` in output.

**Stellar docs**: [Fees, Resource Limits, and Metering — Resource fee](https://developers.stellar.org/docs/learn/fundamentals/fees-resource-limits-metering#resource-fee)

---

## Memory Bytes

- **Tool display name**: `memory bytes` (used by `#[budget_mem_lt(N)]`)
- **SDK field**: `env.cost_estimate().budget().memory_bytes_cost()`
- **What it is**: The host's accounting of memory allocated during execution. Memory bytes are metered alongside CPU instructions during host and guest execution — but, unlike CPU instructions, memory usage is *not* included in fee computation. It is subject to a per-transaction cap (a resource limit) and exceeding it terminates the transaction.
- **Measurement status**: **does not measure** — `cargo budget-report` does not report memory bytes. The `#[budget_mem_lt(N)]` macro asserts the *local* estimate during `cargo test`.

**Stellar docs**: [Fees, Resource Limits, and Metering — Metering](https://developers.stellar.org/docs/learn/fundamentals/fees-resource-limits-metering#metering) ("memory usage is not included in the fee computation, it is nevertheless subject to the resource limits")

---

## Resource budget

- **Tool display name**: `budget` (in `env.cost_estimate().budget()`)
- **What it is**: A caps object carried by every `Env` that limits CPU instructions and memory bytes. The default test budget (`Env::default()`) has a low cap that can truncate measurement; `reset_unlimited()` removes it so the contract runs to completion and measurement captures the full cost.
- **Measurement status**: **does not measure** — it controls measurement. The macros read the *budget's consumed cost* after the test, and the user must call `reset_unlimited()` before invoking the contract so the budget doesn't cap the execution before all costs are incurred.

**Stellar docs**: [Fees, Resource Limits, and Metering — Metering process](https://developers.stellar.org/docs/learn/fundamentals/fees-resource-limits-metering#metering-process)

---

## Resource limits

- **Stellar documentation term**: "resource limitations" / "per-transaction limit"
- **What it is**: Network-wide caps on each resource type (CPU instructions, ledger entry reads/writes, read bytes, write bytes, transaction size, events & return-value size, and RAM/memory) that a single transaction may consume, regardless of fee. Limits are set by validator consensus and published on the [Stellar Lab Network Limits page](https://lab.stellar.org/network-limits). If a transaction's declared resources exceed a limit, the transaction is rejected before execution.
- **Measurement status**: **does not measure** — `cargo budget-report` reports the resources the transaction *used* (as determined by simulation), not the network-wide limits themselves. The numbers in the report are naturally bounded by these limits.

**Stellar docs**: [Fees, Resource Limits, and Metering — Resource limitations](https://developers.stellar.org/docs/learn/fundamentals/fees-resource-limits-metering#resource-limitations)

---

## Footprint

- **Tool reference**: used in `SorobanTransactionData` XDR (read-only set / read-write set), not surfaced in tool output
- **Stellar documentation term**: "ledger footprint" / "storage footprint"
- **What it is**: The set of ledger keys a transaction declares it will read or write. The footprint is split into a read-only set (`readOnly`) and a read-write set (`readWrite`). The declared footprint bounds what the contract may access; accessing a key outside it causes the transaction to fail. The footprint also determines ledger I/O charges — bytes read from keys in the read-only set and bytes written to keys in the read-write set.
- **Measurement status**: **does not directly measure** — the tool relies on `simulateTransaction` to compute the footprint and includes the resulting bytes in `Read Bytes` / `Write Bytes` fields.

**Stellar docs**: [State Archival — Terms and Semantics](https://developers.stellar.org/docs/learn/fundamentals/contract-development/storage/state-archival#terms-and-semantics); [Transaction Resources](https://developers.stellar.org/docs/learn/fundamentals/contract-development/contract-interactions/stellar-transaction#transaction-resources)

---

## TTL (Time To Live)

- **Stellar documentation term**: "TTL" / "Time To Live"
- **What it is**: The number of ledgers until a contract data entry (persistent, temporary, or instance) or contract code entry is no longer live. When `current_ledger > liveUntilLedger`, the entry becomes archived (persistent/instance) or permanently deleted (temporary). TTL must be periodically extended (paying rent) to keep entries accessible. TTL extensions incur expenditure that contributes to the refundable portion of the resource fee.
- **Measurement status**: **does not measure** — the tool does not report TTL values. Rent and TTL extensions are accounted in the transaction's resource fee, but `cargo budget-report` reports the direct resource consumption (CPU instructions, read bytes, write bytes), not the derived fee or TTL of any entry.

**Stellar docs**: [State Archival — Terms and Semantics](https://developers.stellar.org/docs/learn/fundamentals/contract-development/storage/state-archival#ttl)

---

## Rent

- **Stellar documentation term**: "ledger space rent" / "rent payment"
- **What it is**: The fee charged for extending the TTL of ledger entries (i.e., keeping contract data alive). Rent also covers payments for increasing ledger entry size. Rent fees are *refundable*: the estimated amount is debited from the source account before execution and the difference between estimated and actual rent is refunded afterward.
- **Measurement status**: **does not measure** — rent is a fee type derived from storage usage, not a raw resource count. The tool reports the raw resources (`CPU Instructions`, `Read Bytes`, `Write Bytes`) that feed into fee calculation, not the fee itself.

**Stellar docs**: [Fees, Resource Limits, and Metering — Resource fee](https://developers.stellar.org/docs/learn/fundamentals/fees-resource-limits-metering#resource-fee); [State Archival](https://developers.stellar.org/docs/learn/fundamentals/contract-development/storage/state-archival)

---

## Refundable and non-refundable fees

- **Stellar documentation term**: "refundable fees" / "non-refundable fees"
- **What it is**: The total resource fee is split into two portions:
  - **Non-refundable fees** — charged from CPU instructions, read bytes, write bytes, and transaction bandwidth. These are deducted once and not returned, because the resources were consumed irreversibly.
  - **Refundable fees** — charged from rent, events, and return value size. These are debited upfront (the declared maximum), then the actual usage is measured, and the unused portion is refunded. The transaction *fails* if the declared refundable fee was insufficient to cover actual usage.
- **Measurement status**: **does not directly measure** — `cargo budget-report` reports the resource counts that drive the non-refundable portion (CPU instructions, read bytes, write bytes), not the fees themselves. The CLI does output a summary line: _"The metrics above represent the total unrefundable network execution costs required to run your contract functions."_

**Stellar docs**: [Fees, Resource Limits, and Metering — Refundable and non-refundable resource fees](https://developers.stellar.org/docs/learn/fundamentals/fees-resource-limits-metering#refundable-and-non-refundable-resource-fees)

---

## Ledger state

- **Context**: used in notes about simulation variance
- **What it is**: The on-chain state at the moment of simulation — which ledger entries exist, their current TTLs, and the global ledger-size-driven write fee multiplier. Because simulations run against the current live ledger (whose data changes block-by-block), simulated costs vary slightly between runs. This is why `cargo budget-report` warns: _"These are simulated numbers on testnet and may vary slightly depending on ledger state."_
- **Measurement status**: **not measured** — ledger state is the *environment* the measurement runs in, not a metric the tool extracts.

**Stellar docs**: [Fees, Resource Limits, and Metering — Dynamic pricing for storage](https://developers.stellar.org/docs/learn/fundamentals/fees-resource-limits-metering#dynamic-pricing-for-storage)

---

## Non-refundable resource costs

- **Context**: used in the `mechanics.md` and report summary
- **What it is**: A shorthand for the set of raw resource consumption values — CPU instructions, read bytes, and write bytes — that drive the non-refundable portion of the resource fee. The tool reports these three numeric metrics per function, and the summary line refers to them collectively as "total unrefundable network execution costs."
- **Measurement status**: **measures** — these are the three metrics every `cargo budget-report` row contains.

**Stellar docs**: [Fees, Resource Limits, and Metering — Refundable and non-refundable resource fees](https://developers.stellar.org/docs/learn/fundamentals/fees-resource-limits-metering#refundable-and-non-refundable-resource-fees)

---

## Derivation summary

The three metrics the tool **measures** — all extracted from `SorobanTransactionData` XDR:

| Metric (tool name) | XDR field | Fee category | Stellar term |
|---|---|---|---|
| **CPU Instructions** | `resources.instructions` | Non-refundable | CPU instructions |
| **Read Bytes** | `resources.disk_read_bytes` | Non-refundable | Bytes read from the ledger |
| **Write Bytes** | `resources.write_bytes` | Non-refundable | Bytes written to the ledger |

Memory bytes (`#[budget_mem_lt(N)]`) are metered by the host but reported only by the local macro, not by the CLI.

All terms not in the table above — resource limits, footprint, TTL, rent, refundable fees, and ledger state — are **not measured** by this tool. They are either part of Stellar's fee derivation (rent, events structures) or environmental constraints the measurement runs within (limits, ledger state, TTL expiry behavior).