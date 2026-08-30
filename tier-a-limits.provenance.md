# tier-a-limits provenance

## Protocol version

The **function-specific Tier A limits** (rows in the table below) were derived
from a Tier B network measurement on **2026-08-27**.  They are not tied to a
specific protocol number — they reflect the network cost profile at the time
the Tier B report was captured.

The **network-wide resource limits** (`NETWORK__*` keys in
[`tier-a-limits.env`](tier-a-limits.env)) correspond to **Protocol 23**
(Stellar network upgrade activated September 3 2025 on Mainnet, July 17 2025
on Testnet).  These values are documented on the Stellar Lab
[Network Limits](https://lab.stellar.org/network-limits) page and can be
queried via the Stellar CLI (`stellar network settings`).

> **⚠️ Staleness warning (2026-08-29):**  The Stellar network has progressed
> beyond Protocol 23.  As of August 2026, the Stellar Core repository shows
> releases up to **Protocol 28** (v28.0.0, released August 13 2026).  The
> `NETWORK__*` values below were recorded against Protocol 23 and are likely
> stale.  They should be refreshed against the current network settings before
> relying on percentage-based budget assertions.

## How to refresh the function-specific Tier A limits

1. Generate a Tier B (network-verified) report:

   ```bash
   cargo budget-report --json > build/budget-report.json
   ```

   This requires a Stellar CLI installation, a funded source identity, and
   network access (testnet or futurenet).  Without these, the command will
   fail.

2. Derive Tier A limits from the Tier B report:

   ```bash
   cargo budget-report \
     --derive-limits tier-a-limits.env \
     --from build/budget-report.json
   ```

   This overwrites `tier-a-limits.env` and generates
   `tier-a-limits.provenance.md` (this file) as a sidecar.  The derivation
   applies the per-metric margins from the `[margin]` block in `budget.toml`.

   **Note:** The `--derive-limits` command only regenerates function-specific
   Tier A limits (`TIER_A__*` keys).  It does **not** update the
   `NETWORK__*` keys.  Those must be updated manually (see below).

3. Run the workspace tests to confirm the new limits are valid:

   ```bash
   cargo test --workspace
   ```

## How to refresh the network-wide limits (`NETWORK__*`)

The `NETWORK__*` values are **not** produced by `--derive-limits`.  They must
be updated manually when the target protocol version changes:

1. Look up the current per-transaction resource limits on the Stellar Lab
   [Network Limits](https://lab.stellar.org/network-limits) page, or run:

   ```bash
   stellar network settings
   ```

2. Update the four `NETWORK__*` keys in `tier-a-limits.env`:

   ```
   NETWORK__CPU=<instructions>
   NETWORK__MEM=<bytes>
   NETWORK__DISK_READ_BYTES=<bytes>
   NETWORK__DISK_WRITE_BYTES=<bytes>
   ```

   The key names changed in Protocol 23 (CAP-0066) — `readBytes` became
   `diskReadBytes` because in-memory Soroban reads no longer count against a
   byte limit.

3. Run the workspace tests to confirm the percentage-based assertions still
   pass.

## How to tell whether the numbers are stale

- **Function-specific Tier A limits:** Check the `Generated at (UTC)`
  timestamp at the top of the source file or in the `#`-comment header of
  `tier-a-limits.env`.  If the timestamp is significantly older than the last
  SDK bump or contract change, re-derive from a fresh Tier B report.

- **Network-wide limits (`NETWORK__*`):** Compare the values in
  `tier-a-limits.env` against the Stellar Lab
  [Network Limits](https://lab.stellar.org/network-limits) page or the
  output of `stellar network settings`.  If any value differs, the
  `tier-a-limits.env` values are stale and should be updated.

- **General indicator:** A PR that changes the contract source, the
  `soroban-sdk` version, or the release profile without re-deriving Tier A
  limits is a strong sign the limits may be stale.  See the
  [Deriving Limits](docs/src/deriving_limits.md#when-to-re-derive) guide for
  the full checklist.

## Attempted refresh (2026-08-29)

On 2026-08-29 I actually followed the documented refresh procedure.  Here is
what happened at each step:

### 1. Generate a Tier B report

```bash
cargo budget-report --json > build/budget-report.json
```

**Result: FAILED.**  The workspace cannot parse on the pinned Rust 1.91.0
toolchain because `[profile.release.package.cargo-budget-report]` in the
workspace `Cargo.toml` specifies `panic = "unwind"`, which is not permitted
in per-package profiles until Rust 1.92.  The error message is:

```
error: failed to parse manifest at `…/Cargo.toml`

Caused by:
  `panic` may not be specified in a `package` profile
```

This is a **pre-existing build issue** — it is not caused by this
documentation change.  The Tier B → Tier A derivation pipeline is
structurally correct (verified by reading `cargo-budget-report/src/derive.rs`),
but it cannot currently execute on this toolchain.

### 2. Stellar Lab Network Limits page

**Result: INACCESSIBLE for verification.**  The page at
https://lab.stellar.org/network-limits is a JavaScript-rendered SPA; the
network-limit values are not present in the initial HTML and cannot be
verified programmatically or via a simple HTTP request.

### 3. `stellar network settings`

**Result: NOT ATTEMPTED.**  The Stellar CLI is not installed in this
environment.

### 4. CAP-0066 / Protocol 23 XDR diff

**Result: SUCCESS.**  Reviewed the XDR changes in
https://github.com/stellar/stellar-protocol/blob/master/core/cap-0066.md.
Confirmed that Protocol 23 renamed `readBytes` to `diskReadBytes` in
`SorobanResources`, consistent with the `DISK_READ_BYTES` key name already
used in `tier-a-limits.env`.

### 5. Protocol version check

**Result: Staleness detected.**  Checked the Stellar Core releases page
(https://github.com/stellar/stellar-core/releases) and found that the
network has progressed well beyond Protocol 23.  As of August 2026, the
latest release is **v28.0.0** (Protocol 28, released August 13 2026),
with Protocol 27 (June 5 2026) and Protocol 26 (April 2026) also shipped
in the interim.  The `NETWORK__*` values in `tier-a-limits.env` correspond
to Protocol 23 and are almost certainly stale.

### Summary

The refresh procedure itself (`cargo budget-report --json | cargo budget-report
--derive-limits`) is structurally sound — verified by reading the derivation
source in `cargo-budget-report/src/derive.rs`.  The barrier is the build
failure on the current Rust toolchain (pre-existing, not caused by this
change), not a flaw in the procedure.

## Source table

- Source Tier B JSON: `/tmp/tier_b_report.json`
- Margins (cpu, memory, read, write): `1.5000`, `1.2500`, `2.0000`, `3.0000`
- Generated at (UTC): `2026-08-27T08:13:22Z`

This file is auto-generated. Re-run `cargo budget-report --derive-limits` to refresh. The columns are the inputs and result of every Tier A limit; `tier_a_limit = ceil(tier_b_value × margin_metric)`.

## See also

- [Deriving Tier A limits](docs/src/deriving_limits.md) — the re-derivation workflow, when to re-derive, and what to do when a limit moves.
- [Tool Reference: Macros](docs/src/reference.md#macros-budget_macros) — the budget macro attributes and their limit-source forms.
- [Network limits and percentage-based assertions](docs/src/deriving_limits.md#network-limits-and-percentage-based-assertions) — how the `NETWORK__*` keys feed `pct = N, of = …` assertions.

| Key | Tier B value | Margin | Tier A limit |
|---|---:|---:|---:|
| `TIER_A__AMM_POOL_CONTRACT__DO_EVENT_HEAVY_WORK__CPU` | 1627002 | 1.5000 | 2440503 |
| `TIER_A__AMM_POOL_CONTRACT__DO_EVENT_HEAVY_WORK__READ` | 0 | 2.0000 | 0 |
| `TIER_A__AMM_POOL_CONTRACT__DO_EVENT_HEAVY_WORK__WRITE` | 0 | 3.0000 | 0 |
| `TIER_A__AMM_POOL_CONTRACT__DO_EXPENSIVE_WORK__CPU` | 1945128 | 1.5000 | 2917692 |
| `TIER_A__AMM_POOL_CONTRACT__DO_EXPENSIVE_WORK__READ` | 0 | 2.0000 | 0 |
| `TIER_A__AMM_POOL_CONTRACT__DO_EXPENSIVE_WORK__WRITE` | 932 | 3.0000 | 2796 |
| `TIER_A__AMM_POOL_CONTRACT__INITIALIZE__CPU` | 1578562 | 1.5000 | 2367843 |
| `TIER_A__AMM_POOL_CONTRACT__INITIALIZE__READ` | 0 | 2.0000 | 0 |
| `TIER_A__AMM_POOL_CONTRACT__INITIALIZE__WRITE` | 208 | 3.0000 | 624 |
| `TIER_A__AMM_POOL_CONTRACT__NOOP__CPU` | 1542328 | 1.5000 | 2313492 |
| `TIER_A__AMM_POOL_CONTRACT__NOOP__READ` | 0 | 2.0000 | 0 |
| `TIER_A__AMM_POOL_CONTRACT__NOOP__WRITE` | 0 | 3.0000 | 0 |
| `TIER_A__AMM_POOL_CONTRACT__REQUIRE_AUTH_ONLY__CPU` | 1559174 | 1.5000 | 2338761 |
| `TIER_A__AMM_POOL_CONTRACT__REQUIRE_AUTH_ONLY__READ` | 0 | 2.0000 | 0 |
| `TIER_A__AMM_POOL_CONTRACT__REQUIRE_AUTH_ONLY__WRITE` | 0 | 3.0000 | 0 |
