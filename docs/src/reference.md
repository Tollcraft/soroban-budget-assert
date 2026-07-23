# Tool Reference

## Macros: `budget_macros`

Both macros are attribute macros for test functions. They require a local variable named `env` (a `soroban_sdk::Env`) in the function body — the injected check reads `env.cost_estimate().budget()` after the original test statements run.

### `#[budget_cpu_lt(N)]`

Asserts that the CPU instruction cost measured by the test's `env` is strictly less than `N`.

- `N` is an integer literal (e.g., `850000`).
- On failure the test panics with:
  `CPU instruction cost {actual} exceeded limit {N} - local estimate, underestimates real network cost`

```rust
use budget_macros::budget_cpu_lt;

#[test]
#[budget_cpu_lt(850000)]
fn test_expensive_function() {
    let env = Env::default();
    // ... register contract as WASM, call client ...
}
```

### `#[budget_mem_lt(N)]`

Same shape; asserts `memory_bytes_cost() < N`.

### Requirements and caveats

{% hint style="warning" %}
- The variable must be named `env`. The macro resolves the identifier by name.
- Run the contract as WASM (`env.register_contract_wasm`) inside the test, not as raw Rust — raw Rust estimates ran ~81% under real network cost in our measurements and make the assertion meaningless.
- Call `env.cost_estimate().budget().reset_unlimited()` before invoking the contract so measurement isn't cut short by the default test budget.
- The macro checks the *local* estimate, which can sit above or below the real network cost depending on the build profile. Set `N` a few percent above the measured local number to catch regressions, and use `cargo budget-report` for the network ground truth (see the End-User Guide).
{% endhint %}

## CLI: `cargo budget-report`

```
cargo budget-report [--network <network>] [--source <source>] [--json]
```

| Flag | Required | Meaning |
|---|---|---|
| `--network` | yes (flag or `budget.toml`) | Network to deploy and simulate against, e.g. `testnet` |
| `--source` | yes (flag or `budget.toml`) | Funded identity used for deploy fees and as the simulation source |
| `--json` | no | Emit the report as pretty-printed JSON instead of a table |

Configuration precedence: a CLI flag overrides the `budget.toml` value. If neither provides `network`/`source`, the command exits with an error naming the missing field.

External requirements: the `stellar` CLI on `PATH`, a funded source identity on the target network, and the `wasm32-unknown-unknown` Rust target installed.

## Configuration: `budget.toml`

Read from the directory the command runs in (the workspace root):

{% code title="budget.toml" %}
```toml
network = "testnet"
source = "alice"

# Per-function invoke arguments, passed to `stellar contract invoke -- <fn> <args>`
[functions.do_expensive_work]
args = ["--n", "10000"]
```
{% endcode %}

- `network`, `source` — defaults for the corresponding CLI flags.
- `[functions.<name>].args` — arguments injected when simulating that exported function. Functions without an entry are simulated with no arguments; if a required argument is missing, the simulation fails with a warning and that function is skipped (the run continues).

## Output

Each simulated function produces three rows (or three JSON objects): `CPU Instructions`, `Read Bytes`, and `Write Bytes`.

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

## Measurement scope

`cargo budget-report` reports **resource amounts from a simulation, not fees**. It reads three
fields out of the `SorobanTransactionData` returned by `simulateTransaction` —
`resources.instructions`, `resources.disk_read_bytes`, and `resources.write_bytes` — and prints
them unchanged. Nothing in the output is denominated in stroops, and no figure it prints is a
total.

### In scope

| Reported | Stellar resource it corresponds to |
|---|---|
| `CPU Instructions` | `resources.instructions` — metered CPU instruction count |
| `Read Bytes` | `resources.disk_read_bytes` — bytes read from disk-backed ledger entries |
| `Write Bytes` | `resources.write_bytes` — bytes written to ledger entries |

These three quantities are *inputs* to the **non-refundable resource fee**. They are not the
whole of it.

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
- **WASM binary size** — the size of the deployed contract binary is not reported. This is
  coming: see issue #88.

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
