## 🔁 Deriving Tier A limits from a Tier B report

Tier A tests are fast, local, and CI-blocking — but the values they assert
are ultimately down to a developer's reading of a Tier B number plus a margin
applied in their head. Hand-tuning rots as soon as the contract (or the
protocol) changes, and the reconciliation comments drift out of date within a
few commits.

This branch (`feat/derive-tier-a-limits-from-tier-b`) wires those two halves
together with a single command and a checked-in artifact. The Tier A test
annotations read limits out of a `KEY=VALUE` file at runtime, and a CLI
sub-command regenerates that file from a network-verified `cargo
budget-report --json` output, with the margin recorded as data instead of
buried in human reasoning.

{% hint style="info" %}
**Baselines and marginal cost.** Tier A macro assertions use the
`baseline = <expr>` parameter to subtract WASM instantiation overhead from
the raw local measurement before comparing against the limit. The Tier B
network limits derived by `--derive-limits` do *not* include this floor, so
the two are already aligned: the derived limit is compared against the
marginal cost, not the raw measurement. See
[Marginal-cost baseline subtraction](reference.md#marginal-cost-baseline-subtraction)
in the Tool Reference for the full explanation of what the baseline is, how
it is measured, and what the resulting number represents.
{% endhint %}

### One-time setup

Add a `[margin]` block to `budget.toml` so the derivation tool can read the
multipliers without CLI flags:

```toml
[margin]
cpu_margin    = 1.50
memory_margin = 1.25
read_margin   = 2.00
write_margin  = 3.00
```

All four fields are required. The per-metric split is the minimum
granularity that fights back against [issue #45](#related-issues): a single
global margin is wrong across operation types because the local-vs-network
gap has different shapes for host-calls vs. VM loops.

For tests that exercise multi-step workflows (e.g. `test_budget_macro_gated`,
which invokes `deposit + swap + withdraw` in a single test), declare the
component set under `[scenarios.<name>]` so the derivation tool emits one
`KEY=VALUE` per metric for the entire scenario:

```toml
[scenarios.full_workflow]
package = "amm-pool-contract"
functions = ["deposit", "swap", "withdraw"]
```

The tool will emit `TIER_A__AMM_POOL_CONTRACT__SCENARIO__FULL_WORKFLOW__CPU`
= `ceil((deposit_cpu + swap_cpu + withdraw_cpu) × cpu_margin)`, alongside the
per-function `KEY=VALUE` rows.

### Re-derivation workflow

```bash
# 1) Refresh the Tier B report (network-verified ground truth).
cargo budget-report --json > build/budget-report.json

# 2) Regenerate the Tier A limit artifact from this Tier B input.
cargo budget-report \
  --derive-limits tier-a-limits.env \
  --from build/budget-report.json

# (Or pipe straight from --json into the derive step.)
cargo budget-report --json | cargo budget-report \
  --derive-limits tier-a-limits.env --from -

# 3) Run the workspace tests. The Tier A assertions read from
#    tier-a-limits.env at runtime via the macro's
#    `env_file = "PATH"` + `env = "VAR"` form.
cargo test --workspace
```

The CLI emits two companions next to `<OUT>`:

- `<OUT>.provenance.md` — a Markdown table that pairs every Tier A limit
  with its `(tier_b_value, margin)` inputs. Reviewers read this in PR diffs
  to see exactly which Tier B number produced which Tier A limit.
- A header block in `<OUT>` itself — the same provenance as
  `#` comments so non-Rust tooling can grep it.

Both files begin with `# tier-a-limits.env`, `# tier-a-limits provenance`
respectively, and are atomically replaced on each write.  The provenance
file also documents the protocol version, refresh procedure, and how to
detect staleness — see
[`tier-a-limits.provenance.md`](../../tier-a-limits.provenance.md).

### When to re-derive

Re-run `cargo budget-report --derive-limits` whenever **any** of the
following changes, in roughly decreasing order of urgency:

1. The contract source (any code path that produces a Tier A regression
   in CI is a sign that the Tier B report's underlying profile also moved).
2. The release profile in the workspace's `Cargo.toml` — see
   [_Use the same release profile for comparable numbers_](#use-the-same-release-profile-for-comparable-numbers)
   above; an `opt-level` or `lto` flip silently re-prices every limit.
3. The `soroban-sdk` or `stellar-xdr` version (different host metering,
   different VM cost model; see `MEASUREMENTS.md` for SDK-versioned
   calibration).
4. The margin values in `budget.toml` — usually because a new operation
   type lands with a different local-vs-network gap.
5. The target protocol version — network-wide resource limits (CPU
   instructions, memory, disk read/write bytes) are set by validator
   consensus and may change across protocol upgrades. The
   `NETWORK__*` keys in `tier-a-limits.env` record these limits; see
   [Network limits and percentage-based assertions](#network-limits-and-percentage-based-assertions)
   below.

For routine maintenance, treat the margin block as a stable input: change a
margin once, in a PR that explains why, and let the resulting Tier B → Tier
A re-derivation flow into git as the worked audit trail.

### What to do when a limit moves

A diff in `tier-a-limits.env` is **not** automatically correct. Walk through:

1. Look at [`tier-a-limits.provenance.md`](../../tier-a-limits.provenance.md). Same `tier_b_value`, higher
   `tier_a_limit`? The Tier A assertion was too loose and you've widened
   it. Tighten the limit by hand only if you understand why Tier B
   hasn't grown the same way; otherwise update the margin in
   `budget.toml` and re-derive.
2. Same `tier_b_value`, lower `tier_a_limit`? This is the regression case.
   Inspect the Tier A test — if WASM local has dropped below the Tier B
   ceiling, you have a headroom win; if WASM local has fallen below the
   new limit only because the Tier B measurement moved, accept the new
   Tier A cap and that's the workflow working as designed.
3. Different `tier_b_value`, same `margin`? Either the contract grew (so
   re-derive is healthy) or `cargo budget-report --json` returned a
   different value for a non-deterministic reason (ledger state, build
   cache); re-run to disambiguate.

If a limit surprises you, do **not** edit `tier-a-limits.env` by hand —
that erases the provenance and breaks the audit trail. Re-run
`--derive-limits` against a fresh report and let the new numbers land.

### Related issues

This change pairs with two open issues that sit outside its scope but
consume the same primitives:

- **Issue #45 — per-operation-type margin.** A single `[margin]` block
  applies the same multiplier to every function and metric. Per-function
  overrides would slot into the existing `(package, function)` index that
  the derivation tool already iterates over; the TODO is in
  `cargo-budget-report/src/derive.rs::Margin::for_metric`. The path is to
  carry `Margin::defaults` plus a `margin_overrides: HashMap<Key, f64>`
  through `DerivationConfig`, no macro changes required.
- **Issue #10 — baseline / regression mode.** `cargo budget-report
  --record-baseline <FILE>` already records the Tier B shapes into a
  TOML baseline, and `--check-baseline <FILE>` enforces per-metric
  tolerance against it. The two modes complement each other: use
  `--derive-limits` to establish or refresh a Tier A artifact from a
  ground-truth Tier B measurement, then use `--record-baseline` to
  pin the Tier B that the artisan decision was based on, so a future
  rerun can detect when the Tier B itself moves.

### Network limits and percentage-based assertions

`tier-a-limits.env` also records the Soroban network-wide resource limits
under `NETWORK__*` keys. These are per-transaction caps set by validator
consensus and may change across protocol versions:

| Key | Description | Protocol 23 value |
|---|---|---|
| `NETWORK__CPU` | Max CPU instructions per transaction | 100,000,000 |
| `NETWORK__MEM` | Max memory bytes per transaction | 41,943,040 |
| `NETWORK__DISK_READ_BYTES` | Max bytes read per transaction | 200,000 |
| `NETWORK__DISK_WRITE_BYTES` | Max bytes written per transaction | 132,096 |

These limits are documented in the Stellar Lab
[Network Limits](https://lab.stellar.org/network-limits) page and can be
queried via `stellar network settings`.

The budget macros' `pct = N, of = ...` form reads these values to resolve
percentage-based limits. Instead of hard-coding an absolute number that
goes stale when the protocol upgrades, you express intent:

```rust
#[test]
#[budget_cpu_lt(pct = 25, of = env_file = "tier-a-limits.env", env = "NETWORK__CPU")]
fn test_cpu_stays_under_quarter_of_network() {
    let env = Env::default();
    // ... the assertion resolves to 25% of NETWORK__CPU at test runtime
}
```

When the target protocol changes, update the `NETWORK__*` values in
`tier-a-limits.env` (or re-run `cargo budget-report --derive-limits` if
the toolchain fetches live network limits) and every percentage-based
assertion automatically adapts — no test annotations need updating.

