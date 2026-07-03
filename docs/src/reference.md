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

- The variable must be named `env`. The macro resolves the identifier by name.
- Run the contract as WASM (`env.register_contract_wasm`) inside the test, not as raw Rust — raw Rust estimates ran ~81% under real network cost in our measurements and make the assertion meaningless.
- Call `env.cost_estimate().budget().reset_unlimited()` before invoking the contract so measurement isn't cut short by the default test budget.
- The macro checks the *local* estimate, which can sit above or below the real network cost depending on the build profile. Set `N` a few percent above the measured local number to catch regressions, and use `cargo budget-report` for the network ground truth (see the End-User Guide).

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

```toml
network = "testnet"
source = "alice"

# Per-function invoke arguments, passed to `stellar contract invoke -- <fn> <args>`
[functions.do_expensive_work]
args = ["--n", "10000"]
```

- `network`, `source` — defaults for the corresponding CLI flags.
- `[functions.<name>].args` — arguments injected when simulating that exported function. Functions without an entry are simulated with no arguments; if a required argument is missing, the simulation fails with a warning and that function is skipped (the run continues).

## Output

Each simulated function produces three rows (or three JSON objects): `CPU Instructions`, `Read Bytes`, and `Write Bytes`.

Table output ends with a note that values are testnet simulations and vary slightly with ledger state. JSON output (`--json`) is an array suited to CI:

```json
[
  {
    "package": "example-contract",
    "function": "do_expensive_work",
    "metric": "CPU Instructions",
    "value": 756678
  }
]
```

## Failure behavior

- Build failure, deploy failure, or an unparsable RPC response aborts the run with a contextual error (via `anyhow`) — e.g., a deploy failure reports that the source account may be unfunded.
- A failed simulation of a single function prints a warning and skips it; the report still prints for the functions that succeeded.
- If nothing simulates successfully, the CLI prints `No successful simulations to report.` and exits 0.
