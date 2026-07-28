# Tool Reference

## Macros: `budget_macros`

Both macros are attribute macros for test functions. They require a local variable named `env` (a `soroban_sdk::Env`) in the function body — the injected check reads `env.cost_estimate().budget()` after the original test statements run.

The check runs on every path that leaves the test, so all of these body shapes work:

```rust
#[test]
#[budget_cpu_lt(850000)]
fn unit_test() {
    let env = Env::default();
    // ... the check runs after the last statement ...
}

#[test]
#[budget_cpu_lt(850000)]
fn result_test() -> Result<(), Box<dyn std::error::Error>> {
    let env = Env::default();
    let wasm = std::fs::read("../target/wasm32-unknown-unknown/release/my_contract.wasm")?;
    // ... the check runs after `Ok(())` is evaluated, and it is still the test's value ...
    Ok(())
}

#[test]
#[budget_cpu_lt(850000)]
fn early_return_test() {
    let env = Env::default();
    if std::env::var("SKIP_SLOW_PATH").is_ok() {
        return; // the check runs here too
    }
    // ...
}
```

### `#[budget_cpu_lt(N)]`

Asserts that the CPU instruction cost measured by the test's `env` is strictly less than `N`.

- `N` is an integer literal (e.g., `850000`).
- On failure the test panics with:
  `CPU instruction cost {actual} exceeded limit {N} - local estimate, real network cost may differ significantly in either direction`

```rust
use budget_macros::budget_cpu_lt;
use soroban_sdk::Env;

#[test]
#[budget_cpu_lt(950000)] // local WASM ~901,816; testnet ~756,678
fn test_expensive_function() {
    let env = Env::default();

    let wasm = std::fs::read(
        "../target/wasm32-unknown-unknown/release/my_contract.wasm",
    ).expect("build the WASM first");
    let contract_id = env.register_contract_wasm(None, wasm.as_slice());
    let client = MyContractClient::new(&env, &contract_id);

    env.cost_estimate().budget().reset_unlimited();
    client.do_expensive_work(&10_000);
}
```

**Dynamic limit** — read the limit from an environment variable at test time:

```rust
#[test]
#[budget_cpu_lt(env = "MY_CPU_LIMIT")]
fn test_with_env_limit() {
    std::env::set_var("MY_CPU_LIMIT", "850000");
    let env = Env::default();
    // ... test logic ...
}
```

If the environment variable is unset or not a valid `u64`, the limit defaults to `u64::MAX` (effectively disabling the assertion).

**Config-driven limit** — read the limit from a JSON configuration file at test time:

```rust
#[test]
#[budget_cpu_lt(config = "cpu_instructions")]
fn test_with_json_config() {
    let env = Env::default();
    // ... test logic ...
}
```

The macro reads `budget.json` from the current working directory and looks up the value for the given key. The expected file format is:

{% code title="budget.json" %}
```json
{
  "cpu_instructions": 2500000,
  "memory_bytes": 500000
}
```
{% endcode %}

If the file does not exist, the key is missing, or the value is not a valid `u64`, the macro prints a warning and falls back to `u64::MAX` (effectively disabling the assertion). This preserves backwards compatibility — existing tests without a `budget.json` file are unaffected.

On failure the test panics with:
```
CPU instruction cost {actual} exceeded limit {N} - local estimate, real network cost may differ significantly in either direction
```

### `#[budget_mem_lt(N)]`

Asserts that the memory bytes cost measured by the test's `env` is strictly less than `N`.

**Static limit:**

```rust
use budget_macros::budget_mem_lt;

#[test]
#[budget_mem_lt(500000)]
fn test_memory_budget() {
    let env = Env::default();
    // ... register contract as WASM, call client ...
}
```

**Dynamic limit:**

```rust
#[test]
#[budget_mem_lt(env = "MY_MEM_LIMIT")]
fn test_memory_with_env_limit() {
    std::env::set_var("MY_MEM_LIMIT", "500000");
    let env = Env::default();
    // ... register contract as WASM, call client ...
}
```

**Config-driven limit:**

```rust
#[test]
#[budget_mem_lt(config = "memory_bytes")]
fn test_memory_with_json_config() {
    let env = Env::default();
    // ... test logic ...
}
```

Failure message format:
```
Memory bytes cost {actual} exceeded limit {N} - local estimate, real network cost may differ significantly in either direction
```

```rust
use budget_macros::budget_mem_lt;
use soroban_sdk::Env;

#[test]
#[budget_mem_lt(500000)]
fn test_memory_budget() {
    let env = Env::default();

    let wasm = std::fs::read(
        "../target/wasm32-unknown-unknown/release/my_contract.wasm",
    ).expect("build the WASM first");
    let contract_id = env.register_contract_wasm(None, wasm.as_slice());
    let client = MyContractClient::new(&env, &contract_id);

    env.cost_estimate().budget().reset_unlimited();
    client.do_expensive_work(&10_000);
}
```

### Requirements and caveats

{% hint style="warning" %}
- The variable must be named `env`. The macro resolves the identifier by name.
- A `?` that propagates an error leaves the test before the check runs. The test still fails on the returned error, so a regression cannot pass unnoticed — but the budget number is not measured on that path.
- A `return` that comes from *another* macro's expansion (e.g. an `ensure!`/`bail!`-style macro) is invisible to the rewrite and skips the check. A `return` written directly inside macro invocation tokens is rejected with a compile error instead of being skipped silently; move it out of the macro call. This applies to every budget macro.
- `return` inside a closure or `async` block in the test body is left alone — it exits that body, not the test.
- Run the contract as WASM (`env.register_contract_wasm`) inside the test, not as raw Rust — raw Rust estimates ran ~81% under real network cost in our measurements and make the assertion meaningless.
- Call `env.cost_estimate().budget().reset_unlimited()` before invoking the contract so measurement isn't cut short by the default test budget.
- The macro checks the *local* estimate, which can sit above or below the real network cost depending on the build profile. Set `N` a few percent above the measured local number to catch regressions, and use `cargo budget-report` for the network ground truth (see the End-User Guide).
{% endhint %}

## Soroban Budget API

The macros and manual tests interact with the Soroban budget API through `env.cost_estimate().budget()`. The key methods are:

| Method | Returns | Description |
|---|---|---|
| `cpu_instruction_cost()` | `u64` | Total CPU instructions consumed since the last reset |
| `memory_bytes_cost()` | `u64` | Total memory bytes consumed since the last reset |
| `reset_unlimited()` | `()` | Resets all cost counters and removes the default test budget cap |

Example of manual inspection:

```rust
let budget = env.cost_estimate().budget();
budget.reset_unlimited();

// ... invoke contract ...

let cpu = budget.cpu_instruction_cost();
let mem = budget.memory_bytes_cost();
println!("CPU: {cpu}, Memory: {mem}");
```

{% hint style="info" %}
`reset_unlimited()` must be called *before* the contract invocation you want to measure. The default `Env` applies a low test budget that caps measurement if not removed.
{% endhint %}

## CLI: `cargo budget-report`

```
cargo budget-report [--network <network>] [--source <source>] [--json] [--check]
```

| Flag | Required | Meaning |
|---|---|---|
| `--network` | yes (flag or `budget.toml`) | Network to deploy and simulate against, e.g. `testnet` |
| `--source` | yes (flag or `budget.toml`) | Funded identity used for deploy fees and as the simulation source |
| `--json` | no | Emit the report as pretty-printed JSON instead of a table |
| `--check` | no | Compare measured metrics against `cpu_limit` / `read_limit` / `write_limit` declared per function in `budget.toml`; print a per-function+metric pass/fail line and exit non-zero on any breach or failed configured simulation |

Configuration precedence: a CLI flag overrides the `budget.toml` value. If neither provides `network`/`source`, the command exits with an error naming the missing field.

External requirements: the `stellar` CLI on `PATH`, a funded source identity on the target network, and the `wasm32-unknown-unknown` Rust target installed.

### Required release profile for comparable measurements

`cargo budget-report` builds each contract with `cargo build --target wasm32-unknown-unknown --release`, so the workspace `[profile.release]` is part of the measured input. To compare against the figures published by this project, use the same profile:

{% code title="Cargo.toml" %}
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
{% endcode %}

Each setting can move the reported costs: size optimization, LTO, and a single codegen unit affect generated instructions; aborting panics removes unwinding code; strip/debug settings affect WASM bytes; release assertions avoid debug-only work; and overflow checks keep arithmetic checks in the measured release artifact. Results produced with another release profile describe another WASM build and are not comparable. The current tool does not warn when these settings are absent; that is a follow-up to consider rather than behavior implemented here.

### `--check`: enforcing regression limits against network-verified costs

The `--check` flag turns the report into a CI gate. Behavior:

- Each measured metric is compared against its configured limit. A **missing** limit means that metric is **reported but not enforced**.
- A pass/fail line is printed per `function+metric` and a summary line counts how many checks passed and failed.
- `--check` exits with a non-zero status if **any** limit is breached, **or** if a function that has a `budget.toml` entry fails to simulate successfully — a broken simulation can otherwise look like a silent pass.
- Functions that are not declared in `budget.toml` are reported but never checked.
- `--check` composes with `--json`: every JSON entry for a configured function gains `limit` and `pass` fields. Entries with neither field stay byte-for-byte identical to the plain JSON output.

When `--check` is **not** passed, the plain text and JSON output of `cargo budget-report` is unchanged from earlier versions, so existing CI consumers do not need to be updated.

{% code title="budget.toml" %}
```toml
network = "testnet"
source = "alice"

[functions.do_expensive_work]
args = ["--n", "10000"]
cpu_limit = 5000000
read_limit = 5000
write_limit = 1000

# AMM pool functions are local-only; reporting is fine but they are not
# invoked via `cargo budget-report` end-to-end.
```
{% endcode %}

### Plain text output example (`--check`)

```text
=== WORKSPACE BUDGET REPORT ===
... existing per-metric table unchanged ...

Summary: ... unchanged lines ...

=== BUDGET CHECKS ===
amm-pool-contract::do_expensive_work [CPU Instructions] value=1,234,567 inst. limit=5,000,000 inst. PASS
amm-pool-contract::do_expensive_work [Read Bytes] value=2,048 B limit=5,000 B PASS
amm-pool-contract::do_expensive_work [Write Bytes] value=4,096 B limit=1,000 B FAIL
Summary: 2 check(s) passed, 1 failed
```

### JSON output example (`--check --json`)

```json
[
  {
    "package": "amm-pool-contract",
    "function": "do_expensive_work",
    "metric": "CPU Instructions",
    "value": 1234567,
    "limit": 5000000,
    "pass": true
  },
  {
    "package": "amm-pool-contract",
    "function": "do_expensive_work",
    "metric": "Read Bytes",
    "value": 2048,
    "limit": 5000,
    "pass": true
  },
  {
    "package": "amm-pool-contract",
    "function": "do_expensive_work",
    "metric": "Write Bytes",
    "value": 4096,
    "limit": 1000,
    "pass": false
  }
]
```

For a function declared in `budget.toml` whose simulation fails, an entry still appears with `value` omitted and `pass: false`:

```json
{
  "package": "amm-pool-contract",
  "function": "do_expensive_work",
  "metric": "CPU Instructions",
  "limit": 5000000,
  "pass": false
}
```

## Configuration: `budget.toml`

The CLI walks upward from the current directory looking for `budget.toml`. When the file is present at the workspace root, running `cargo budget-report` from any subdirectory (e.g. inside a member crate) still finds it. If no `budget.toml` is found in any ancestor directory the CLI falls back to its defaults (network and source must be supplied via flags).

{% code title="budget.toml" %}
```toml
network = "testnet"
source = "alice"

# Default tolerance for regressions on `--check-baseline`. Functions may
# override this with their own `tolerance`. Accepts the same forms as
# `--tolerance`: either a fraction (0.10) or a percentage ("10%").
tolerance = 0.10

# Per-function invoke arguments, passed to `stellar contract invoke -- <fn> <args>`.
[functions.do_expensive_work]
args = ["--n", "10000"]

# Optional enforcement limits consulted by `cargo budget-report --check`.
# Any field omitted means the metric is reported but not enforced.
cpu_limit = 5000000
read_limit = 5000
write_limit = 1000
```
{% endcode %}

- `network`, `source` — defaults for the corresponding CLI flags.
- `[functions.<name>].args` — arguments injected when simulating that exported function. Functions without an entry are simulated with no arguments; if a required argument is missing, the simulation fails with a warning and that function is skipped.
- `[functions.<name>].cpu_limit`, `.read_limit`, `.write_limit` — inclusive upper bounds for simulated CPU instructions, read bytes, and write bytes. Enforced only when `--check` is passed. A missing field means "not enforced" for that metric.

## Output

Each simulated function produces four rows (or four JSON objects) when its simulation succeeds: `CPU Instructions`, `Read Bytes`, `Write Bytes`, and `WASM Bytes`. For a mapping between these metric names, their XDR field names, and Stellar's own terminology, see the [Cost Terms Glossary](glossary.md).

Table output ends with a note that the values are simulated resource amounts rather than fees,
what is not measured, and that testnet simulations vary slightly with ledger state — see
[Measurement scope](#measurement-scope). JSON output (`--json`) is an array suited to CI:

```json
[
  {
    "package": "amm-pool-contract",
    "function": "do_expensive_work",
    "metric": "CPU Instructions",
    "value": 756678
  }
]
```

When `--check --json` is used, configured functions gain `limit` and `pass` (see [the `--check` section above](#check-enforcing-regression-limits-against-network-verified-costs)); the shape for unconfigured functions is unchanged.

## Measurement scope

`cargo budget-report` reports **resource amounts from a simulation, not fees**. It reads three
fields out of the `SorobanTransactionData` returned by `simulateTransaction` —
`resources.instructions`, `resources.disk_read_bytes`, and `resources.write_bytes` — plus the
compiled WASM binary size from the build step, and prints
them unchanged. On Soroban Protocol 22+ it additionally reads `result.cost.memBytes` from the JSON-RPC `cost` block and surfaces it as a `Memory Bytes` row. Nothing in the output is denominated in stroops, and no figure it prints is a
total.### In scope

| Reported | Stellar resource it corresponds to |
|---|---|
| `CPU Instructions` | `resources.instructions` — metered CPU instruction count |
| `Read Bytes` | `resources.disk_read_bytes` — bytes read from disk-backed ledger entries |
| `Write Bytes` | `resources.write_bytes` — bytes written to ledger entries |
| `WASM Bytes` | Compiled WASM binary size — the file size on disk after `cargo build --target wasm32-unknown-unknown --release` |
| `Memory Bytes` (Protocol 22+) | `result.cost.memBytes` — memory-bytes cost from the Protocol 22 JSON-RPC `cost` block; absent on older protocol responses |

These four (or five on Protocol 22+) quantities are *inputs* to the **non-refundable resource fee**. They are not the whole of it.

### Not in scope

{% hint style="warning" %}
Do not treat the reported numbers as what a transaction will cost. On Stellar, the total
transaction fee is `resource fee + inclusion fee`, and the resource fee is itself
`non-refundable + refundable`. This tool measures neither total, and does not convert what it
measures into a fee.
{% endhint %}

- **Rent** — the fee for creating ledger entries and extending their TTL. Rent is a *refundable*
  resource fee, charged up front and refunded against actual usage. It is frequently the largest
  single line item for a contract that writes persistent state, and it is entirely absent here.
  A simulation surfaces it in the `minResourceFee` and the returned `SorobanTransactionData`
  rent-change data; the [Fees, resource limits, and metering][fees] page explains how it is
  computed.
- **Other refundable fees** — the size of emitted events and of the return value are also
  charged as refundable resource fees. Not measured.
- **Transaction size (bandwidth)** — the serialized transaction and its signatures are charged
  as part of the *non-refundable* resource fee. So even within the non-refundable portion, the
  three reported figures are incomplete.
- **Ledger footprint** — the read-only and read-write entry *keys and counts* in the footprint
  are charged per entry, separately from the byte counts reported here. A function that touches
  many small entries can cost far more than its byte totals suggest. `stellar contract invoke
  --build-only` followed by `stellar xdr decode --type SorobanTransactionData` shows the full
  footprint for a transaction the tool has already built.
- **Total transaction fee** — requires the inclusion fee, which is a bid set by the submitter
  and not a property of the contract at all. The `minResourceFee` field of a
  `simulateTransaction` response is the closest single number to "what the resources cost";
  reach for that, not for this report, when you need a figure in stroops.
### What the report is good for

Comparing a function against itself over time. The three metrics are the ones that move when
contract logic changes, so they are the right signal for catching an execution-cost regression
— which is exactly what the Tier A macros pin into `cargo test`. They are the wrong signal for
answering "how much will my users pay".

[fees]: https://developers.stellar.org/docs/learn/fundamentals/fees-resource-limits-metering

## Failure behavior

- Build failure, deploy failure, or an unparsable RPC response aborts the run with a contextual error (via `anyhow`) — e.g., a deploy failure reports that the source account may be unfunded.
- A failed simulation of a single function prints a warning and skips it; the report still prints for the functions that succeeded.
- If nothing simulates successfully, the CLI prints `No successful simulations to report.` and exits 0.
- When `--check` is passed:
  - Any limit breach exits non-zero.
  - Any function declared in `budget.toml` whose simulation fails also exits non-zero (the warning is still printed), so a broken simulation cannot look like a silent pass.

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
