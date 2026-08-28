# Local vs. Network Cost Gap

When developing Soroban smart contracts, locally measured resource costs (**Tier A**) differ from the costs billed by the Stellar network (**Tier B**). 

Relying on a local estimate as a direct prediction of network cost creates a false sense of security. A budget check set tightly against local numbers can pass consistently in CI right up until it fails on-chain.

This guide explains why this gap exists, how large it typically is across different operation types, which operations remain unmeasured, and how to set practical safety margins for your contract.

> **Canonical evidence:** For the raw benchmark tables, JSON fixtures, SDK version calibration records, and step-by-step reproduction instructions, see the root [`MEASUREMENTS.md`](https://github.com/Tollcraft/soroban-budget-assert/blob/main/MEASUREMENTS.md) file and the [Measurements Reference](measurements.md).

---

## Why Local Estimates Differ from Network Costs

Local tests run your contract inside a local test harness using `Env::cost_estimate().budget()`. Network costs, by contrast, are determined on-chain (or simulated via `simulateTransaction`) using the full host environment metering.

The gap between local estimates and network reality is driven by three main factors:

1. **Host VM vs. Test Harness Execution**: Local tests run in-process within the Rust test runner, whereas network simulations execute within the Soroban Host VM.
   * *Note on Native Rust vs. WASM*: Running tests in native Rust (without registering compiled WASM) underestimates network costs by over 80%. **Only WASM-registered tests produce meaningful budget estimates.**
2. **Build Profile Sensitivity**: The direction of the gap is not stable across compiler profiles. For example, on the same compute and storage benchmark:
   * A size-optimized WASM build (`opt-level = "z"`, LTO enabled) produced a local estimate **19.2% higher** than the network figure (local overestimated).
   * Cargo's default release build (`opt-level = 3`) produced a local estimate **7.8% lower** than the network figure (local underestimated).
3. **SDK and Toolchain Version Shifts**: Internal changes to `soroban-sdk` and host metering models shift local estimates significantly across versions. For instance, upgrading from `soroban-sdk` 22 to 27 reduced local CPU instruction estimates for identical WASM logic by approximately **70%**.

---

## The Gap by Operation Type

The magnitude and direction of the local-vs-network gap vary by operation type:

| Operation Category | Observed Local-vs-Network Behavior | Typical Gap Magnitude |
|---|---|---|
| **VM Arithmetic (Compute-only)** | Local WASM tends to overestimate network CPU cost. | **+8.6%** (local overestimates) |
| **Storage Writes** | Local WASM underestimates network CPU cost for storage writes. | **−17.2%** (local underestimates) |
| **Host-Function Calls** | Local WASM systematically underestimates network CPU cost across host calls (e.g., `sequence()`, `timestamp()`, `sha256()`). | **−6.9% to −19.8%** (local underestimates) |
| **Map Operations** | Local WASM underestimates network cost across insert, get, and iterate operations. | **−3.8% to −10.7%** (local underestimates) |

### Key Scaling Behaviors

* **Compute Loops**: Pure WASM arithmetic loops without host calls add no measurable cost to host budget metering. Budget consumption is driven by host calls and storage operations, not raw WASM loop iterations.
* **Map Operations**: `Map::get` and `Map::iterate` exhibit constant per-operation marginal costs (~3,500 CPU instructions). However, `Map::remove` is **super-linear**: its per-operation cost grows as the map size increases (e.g., growing from ~5,400 CPU instructions at 100 entries to ~9,500 at 1,000 entries).

---

## ⚠️ Uncharacterised & Unmeasured Operations

Not all Soroban operations have verified network deltas. The following operation types currently have pending or uncharacterised network gap figures:

* **Memory Byte Allocations** (e.g., host-resident vector and object allocations)
* **Storage Reads** (isolated from write phases)
* **TTL Extensions** (instance and persistent storage lifetime extensions)
* **Event Emissions** (emitting contract events)
* **Authorization Checks** (`require_auth` calls isolated from contract logic)
* **Cross-Contract Invocations** (multi-contract calls)

> **Warning:** If your contract relies heavily on one of these unmeasured operations, **do not assume the CPU or storage-write gap figures apply**. The cost gap for uncharacterised operations is unknown, and local estimates must be treated with additional safety margins until validated via network simulation.

---

## Practical Guidance: Managing the Gap

### When is a local estimate good enough?

Local WASM assertions (**Tier A**) are fast, deterministic, and ideal for **CI regression gating**. They answer the question: *"Did this pull request accidentally double our CPU usage?"* 

Local numbers are **not** good enough for establishing absolute network budget limits or fee guarantees.

### How to choose safety margins

To protect against unexpected on-chain failures:

1. **Derive Limits from Network Simulations (Recommended)**: Use `cargo budget-report` (Tier B) on Soroban testnet to capture true network baseline figures, then generate Tier A limit files using `cargo budget-report --derive-limits`.
2. **Use Metric-Specific Margins**: Do not use a single flat multiplier for all metrics. Configure separate multipliers in `budget.toml` (`[margin]`):
   ```toml
   [margin]
   cpu_margin    = 1.50   # 50% buffer for CPU instruction drift
   memory_margin = 1.25   # 25% buffer for memory bytes
   read_margin   = 2.00   # 100% buffer for storage read bytes
   write_margin  = 3.00   # 200% buffer for storage write bytes
   ```
3. **Local-Only Safety Buffer**: If you must set a Tier A assertion based purely on local WASM estimates (without a recent testnet report), add at least a **20% to 30% safety margin** above the measured local figure for standard operations, and a larger margin for host-function heavy or `Map::remove` workloads.
4. **Re-derive on Dependencies and Profile Changes**: Re-measure and update limits whenever you bump `soroban-sdk`, update the Rust toolchain, or modify Cargo release profile flags.

---

## Related Documentation

* [End-User Guide](user_guide.md) — Step-by-step setup for budget assertions and reports.
* [Deriving Limits](deriving_limits.md) — How to automatically derive Tier A limits from Tier B network reports.
* [Protocol Mechanics](mechanics.md) — Deep dive into macro instrumentation and CLI simulation pipelines.
* [Measurements Reference](measurements.md) — Project benchmark data and SDK calibration history.
