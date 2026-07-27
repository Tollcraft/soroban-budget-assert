<div align="center">
  <h1>🛡️ Soroban Budget Assert</h1>
  <p><strong>Empirical cost measurement and assertion tooling for Soroban smart contracts.</strong></p>
  
  [![Build Status](https://github.com/Tollcraft/soroban-budget-assert/actions/workflows/budget.yml/badge.svg)](https://github.com/Tollcraft/soroban-budget-assert/actions)
  [![License](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
  <p>
    <a href="https://tollcraft.gitbook.io/docs/budget-assert"><strong>Documentation</strong></a> ·
    <a href="https://tollcraft.github.io/soroban-budget-assert/dashboard.html"><strong>Dashboard</strong></a>
  </p>
</div>

---

## 📖 Overview

`soroban-budget-assert` is a developer tool that measures the gap between local Soroban test estimates and real network costs. It allows developers to assert budget limits during testing and automatically generate detailed execution-resource reports across an entire workspace.

### 🏗️ Architecture

The tool is split into two primary components:

1. **`budget-macros` (Tier A - Local, Fast, CI-Blocking)**
   - Rust macros (`#[budget_cpu_lt(N)]`, `#[budget_mem_lt(N)]`) applied directly to your test functions.
   - Fails the test the moment measured cost crosses your pinned limit, so cost regressions are caught in CI instead of on the network.

2. **`cargo-budget-report` (Tier B - Network-Verified, Reporting)**
   - A CLI tool that automatically discovers all contracts in your workspace.
   - Compiles WASM, simulates execution on testnet, and reports the simulated resource amounts (CPU instructions, read/write bytes) plus the compiled WASM binary size.
   - These are inputs to the non-refundable resource fee — not a total cost. Rent, refundable fees, transaction size, footprint entry counts, and the inclusion fee are not measured; see [Measurement scope](https://tollcraft.gitbook.io/docs/budget-assert/reference#measurement-scope).
   - Configurable via a central `budget.toml` file.

### 🧪 Test Fixture: Constant-Product AMM Pool

The workspace includes `amm-pool-contract`, a constant-product AMM pool fixture that replaces the original `ExpensiveContract` synthetic loop. It exercises the operations that dominate real Soroban costs:

- **Multiple persistent storage keys** — reserves, balances, LP shares, per-user state
- **Authorization** — `require_auth()` on every state-changing operation
- **Event emission** — deposit, swap, and withdraw events
- **Realistic computation** — constant-product math with slippage checks
- **Simulated token flows** — internal balance tracking across pool operations

The fixture is a benchmark, not a product. It implements `initialize`, `deposit`, `swap`, and `withdraw` — enough to produce meaningful cost numbers but small enough to stay readable.

**`do_expensive_work`** is retained as a deliberately named synthetic baseline. Its CPU-bound loop exercises almost none of the host functions that drive real contract costs, making it useful as a comparison point to measure the gap between synthetic benchmarks and realistic contract operations.

## 📊 Cost-over-time Dashboard

Every push to `main` runs [`budget.yml`](.github/workflows/budget.yml), whose `record-history` job appends a `{commit, timestamp, data}` entry to `history.json` on the `gh-pages` branch. The static dashboard at [`site/dashboard.html`](site/dashboard.html) (published by [`deploy-site.yml`](.github/workflows/deploy-site.yml)) fetches that file at page load and plots per-function trend lines, so a regression like "`do_expensive_work` got 12% more expensive over the last ten commits" is visible at a glance.

**How the pieces fit together:**
1. `record-history` job → appends to `history.json` on `gh-pages`.
2. `deploy-site.yml` → publishes `site/**` to `gh-pages` with `keep_files: true`, so `history.json` is never wiped.
3. The dashboard page fetches `history.json` same-origin and pivots it client-side into `package → function → metric` series — no backend, no build-time data baking.

**Using this on your own repo:** copy the `record-history` job pattern and the `site/` folder into your repo, then open the dashboard with query params:
- `?history=URL` — where to fetch `history.json` from (default `./history.json`, same-origin).
- `?repo=owner/name` — links each point to its commit on GitHub (auto-detected on `<owner>.github.io/<repo>/` URLs; set explicitly for custom domains/forks).
- `?limit=N` — how many recent commits to render (default 200).

Example: `https://your-org.github.io/your-repo/dashboard.html?limit=100`.

## ⚙️ Supported Versions & Compatibility

* **Supported SDK Version**: `soroban-sdk` = `"22.0.11"` (specifically tested/resolved to `22.0.11` in `Cargo.lock`)
* **Supported XDR Version**: `stellar-xdr` = `"22.1.0"` (used for decoding transaction simulation responses)
* **Corresponding Stellar Protocol**: **Protocol 22**

### Compatibility Matrix

| SDK Version | Protocol Version | Status | Notes |
| :--- | :--- | :--- | :--- |
| **`< 22.0.0`** | `< 22` | **Untested** | Older protocols may use different transaction/resource schemas. |
| **`22.0.x`** | `22` | **Supported** | Matches pinned manifest dependencies (`soroban-sdk` `22.0.11`, `stellar-xdr` `22.1.0`). |
| **`>= 23.0.0`** | `>= 23` | **Untested** | Future protocol upgrades or XDR schema changes (e.g. key/field renames) may break parsing. |

---

## 🚀 Quick Start

### 1. Installation
Install the CLI tool locally from the repository root:
```bash
cargo install --path cargo-budget-report
```

### 2. Configuration
Scaffold a `budget.toml` in your workspace root:
```bash
cargo budget-report --init
```

This writes a commented template with all available fields and an example
function entry. Review and adjust the values for your project.

To overwrite an existing file, add `--force`:
```bash
cargo budget-report --init --force
```

The `budget.toml` file is shared between both Tollcraft tools —
`cargo-budget-report` and `soroban-cost-linter` — so a single file at the
workspace root serves both tools. Each tool silently ignores sections it
does not own. Unknown keys inside `[functions.*]` blocks produce an error
pointing to the offending key.

Full shared schema:

```toml
# -- cargo-budget-report configuration ----------------------------------------
network = "testnet"           # Target network: "testnet", "futurenet", "local"
source = "alice"              # Stellar source account keypair name

[functions.do_expensive_work]
args = ["--n", "10000"]       # CLI arguments forwarded to the function
cpu_limit = 5000000           # Optional CPU instruction limit (--check)
read_limit = 5000             # Optional read-bytes limit (--check)
write_limit = 1000            # Optional write-bytes limit (--check)

# -- soroban-cost-linter configuration ----------------------------------------
[lints]                       # Consumed by soroban-cost-linter; silently
complexity = "warn"           # accepted by cargo-budget-report.
```

### 3. Usage

**Generate a Workspace Report:**
```bash
cargo budget-report
```

**Use the same release profile for comparable numbers:**

`cargo budget-report` builds contracts with `cargo build --release --target wasm32-unknown-unknown`, so the workspace's `[profile.release]` changes the WASM that gets deployed and simulated. The figures published by this project use the Soroban size-optimized release profile below; copy it into the workspace root before comparing your results to this repo's measurements:

```toml
[profile.release]
opt-level = "z"
overflow-checks = true
debug = 0
strip = "symbols"
debug-assertions = false
panic = "abort"
codegen-units = 1
lto = true
```

These settings are measurement inputs, not cosmetic preferences. `opt-level = "z"` and `lto = true` optimize the generated WASM for size and cross-crate inlining; `codegen-units = 1` gives LLVM a whole-program optimization view; `panic = "abort"` removes unwinding code; `strip = "symbols"` and `debug = 0` remove symbol/debug payload from the artifact; `debug-assertions = false` matches production release behavior; and `overflow-checks = true` keeps arithmetic checks explicit when the release build is measured. Changing any of them can change CPU instructions, memory usage, read/write bytes, or WASM size.

Figures produced under a different release profile are different builds and are not comparable to this project's published cost figures. In the existing fixture, `do_expensive_work(10_000)` measured 901,816 local WASM CPU instructions and 756,678 testnet instructions with the size-optimized profile, but 767,049 local WASM CPU instructions and 832,006 testnet instructions with Cargo's default release profile. A follow-up worth considering is a tool warning when `cargo budget-report` runs in a workspace that lacks these settings.

**Enforce Regression Limits (`--check`):**

Add per-function `cpu_limit`, `read_limit`, and/or `write_limit` to `budget.toml`.
Then run `cargo budget-report --check` — the measured metrics are compared against
the configured limits, a clear pass/fail line is printed per function+metric, and
the process exits non-zero on any breach (or on any configured function whose
simulation fails to run). Functions not declared in `budget.toml` are still
reported but never checked.

```toml
# budget.toml
network = "testnet"
source = "alice"

[functions.do_expensive_work]
args = ["--n", "10000"]
cpu_limit = 5000000
read_limit = 5000
write_limit = 1000
```

```bash
# Plain text report + per-check pass/fail:
cargo budget-report --check

# Same, with machine-readable JSON entries that include `limit` and `pass`
# fields per configured function+metric:
cargo budget-report --check --json
```

### 🛡️ Blocking Network-Cost Regressions in CI

```yaml
# .github/workflows/budget.yml
- name: Build contracts
  run: cargo build -p amm-pool-contract --release --target wasm32-unknown-unknown

- name: Enforce budget limits against network-verified costs
  # Exits non-zero on any limit breach or on any configured function
  # whose simulation fails (so a broken sim cannot look like a pass).
  run: cargo run --bin cargo-budget-report -- budget-report --check --json
```

A pull request that pushes `do_expensive_work` past its limit — for example by
adding an unbounded loop — fails the job with output similar to:

```text
=== BUDGET CHECKS ===
amm-pool-contract::do_expensive_work [CPU Instructions] value=5,400,123 inst. limit=5,000,000 inst. FAIL
amm-pool-contract::do_expensive_work [Read Bytes] value=2,048 B limit=5,000 B PASS
amm-pool-contract::do_expensive_work [Write Bytes] value=1,024 B limit=1,000 B FAIL
Summary: 1 check(s) passed, 2 failed
```

CI surfaces the exact metric and limit on the failing run. Re-measure with
`cargo budget-report` and either optimize the function or consciously raise
the limit.

**Use Macros in Tests:**

The macros (`budget_cpu_lt`, `budget_mem_lt`) are attribute macros for test functions. They require a local variable named **`env`** — the generated code reads `env.cost_estimate().budget()` by name.

```rust
use budget_macros::{budget_cpu_lt, budget_mem_lt};
use soroban_sdk::Env;

// CPU instruction assertion. The limit is read at test runtime from a
// `KEY=VALUE` file generated by `cargo budget-report --derive-limits`
// (see the "Deriving Tier A limits from a Tier B report" section below).
#[test]
#[budget_cpu_lt(env_file = "../tier-a-limits.env",
               env = "TIER_A__AMM_POOL_CONTRACT__SCENARIO__FULL_WORKFLOW__CPU")]
fn test_cpu_budget() {
    let env = Env::default();
    let contract_id = env.register(ConstantProductPool, ());
    let client = ConstantProductPoolClient::new(&env, &contract_id);
    // ... initialize + reset_unlimited + deposit + swap + withdraw ...
}
```

The macros also accept a literal integer, an `env = "VAR"` (process
environment), and `config = "key"` (a `budget.json` file in the
working directory); see `budget-macros/src/lib.rs` rustdoc for the full
form catalogue. The `env_file` form is the recommended form for
network-derived limits because it is thread-safe and review-friendly.

---

## 📊 Measurements

The [MEASUREMENTS.md](MEASUREMENTS.md) file at the repository root records all empirical cost measurements comparing local Soroban budget estimates against real network costs. The [Protocol Mechanics documentation](https://tollcraft.gitbook.io/docs/budget-assert/mechanics) cites this file as the source of truth for measured figures.

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
respectively, and are atomically replaced on each write.

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

For routine maintenance, treat the margin block as a stable input: change a
margin once, in a PR that explains why, and let the resulting Tier B → Tier
A re-derivation flow into git as the worked audit trail.

### What to do when a limit moves

A diff in `tier-a-limits.env` is **not** automatically correct. Walk through:

1. Look at `tier-a-limits.provenance.md`. Same `tier_b_value`, higher
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

## 🤝 Community & Maintainers

Join the discussion and get support:
* **Community Link**: [Stellar Developer Discord](https://discord.gg/5aprtMSyR)

| Maintainer | Role | Telegram |
|------------|------|----------|
| Tollcraft Team | Core Developers | [@tollcraft](https://t.me/+Gflo5jZStw1jMjE0) |

---

## 🛠️ Contributing

We welcome contributions! Please see our [CONTRIBUTING.md](CONTRIBUTING.md) for details on how to get started, and our [SECURITY.md](SECURITY.md) for reporting vulnerabilities.

### 🧑‍💻 Contributors

[![Contributors](https://contrib.rocks/image?repo=Tollcraft/soroban-budget-assert)](https://github.com/Tollcraft/soroban-budget-assert/graphs/contributors)
