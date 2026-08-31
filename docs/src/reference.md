# Tool Reference

## Macros: `budget_macros`

The budget macros are attribute macros that inject budget-measurement assertions
into test functions.  They require a local variable named `env` (a
`soroban_sdk::Env`) — the injected check reads `env.cost_estimate().budget()`
after the original test statements run.

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
### `#[budget_write_bytes_lt(N)]`

Asserts that the ledger write bytes used by `env` are strictly less than `N`.

Write bytes represent the total bytes written to ledger storage during contract execution. This macro measures the local `memory_bytes_cost` as a proxy, which correlates with storage serialization overhead even though the exact on-network write-bytes figure is only available via RPC simulation.

```rust
use budget_macros::budget_write_bytes_lt;

#[test]
#[budget_write_bytes_lt(4096)]
fn test_write_bytes_budget() {
    let env = Env::default();
    // ...
}
```

### `#[budget_read_bytes_lt(N)]`

Asserts that the ledger read bytes used by `env` are strictly less than `N`.

Read bytes represent the total bytes read from ledger storage during contract execution. This macro measures the local `memory_bytes_cost` as a proxy, which correlates with storage access overhead even though the exact on-network read-bytes figure is only available via RPC simulation.

**Static limit:**

```rust
use budget_macros::budget_read_bytes_lt;

#[test]
#[budget_read_bytes_lt(4096)]
fn test_read_bytes_budget() {
    let env = Env::default();
    // ...
}
```

**Dynamic limit:**

```rust
#[test]
#[budget_read_bytes_lt(env = "MAX_READ_BYTES")]
fn test_read_bytes_with_env_limit() {
    let env = Env::default();
    // ...
}
```

**Limit from a `.env` file:**

```rust
#[test]
#[budget_read_bytes_lt(env_file = "../tier-a-limits.env", env = "TIER_A__AMM_POOL_CONTRACT__DEPOSIT__READ")]
fn test_read_bytes_with_env_file() {
    let env = Env::default();
    // ...
}
```

**Config-driven limit** — read from `budget.json` in the process working directory:

```rust
#[test]
#[budget_read_bytes_lt(config = "read_bytes")]
fn test_read_bytes_with_json_config() {
    let env = Env::default();
    // ...
}
```

**Baseline subtraction** — `baseline = <expr>` subtracts a fixed floor from the
measurement before it is compared, so the *marginal* read-bytes cost is what is
asserted, exactly as for `budget_cpu_lt` / `budget_mem_lt` / `budget_write_bytes_lt`.
The subtraction saturates at 0.

```rust
#[test]
#[budget_read_bytes_lt(4096, baseline = instantiation_floor_read_bytes())]
fn test_marginal_read_bytes() {
    let env = Env::default();
    // ...
}
```

Failure message format:
```
Read bytes cost (memory proxy) {actual} exceeded limit {N} - local estimate, underestimates real network cost
```
and with a baseline:
```
Read bytes cost (memory proxy) {marginal} exceeded limit {N} (marginal: {measured} measured - {baseline} baseline) - local estimate, underestimates real network cost
```

### `#[budget_scaling(…)]` — growth-model assertion

Asserts that the CPU cost *grows* according to a declared model as input size
increases.  This is a multi-point assertion: the macro measures the annotated
function at several caller-provided sizes and validates the cost-growth curve.

```rust
use budget_macros::budget_scaling;
use soroban_sdk::Env;

#[budget_scaling(
    sizes = [10, 100, 1000],
    model = linear,
    tolerance = 0.3,
)]
fn operation_scales_linearly(env: Env, size: u32) {
    // body runs once per input size with `env` and `size` in scope
}
```

**Attribute fields:**

| Field       | Type              | Description |
|-------------|-------------------|-------------|
| `sizes`     | `[u32; N]` (N≥2) | Input sizes to measure. |
| `model`     | `linear` / `quadratic` | Expected growth model. |
| `tolerance` | `f64`             | Max allowed relative deviation (e.g. `0.3` = 30%). |

**How it works:**

1. For each `size` in `sizes` a fresh `Env` is created and its budget reset.
2. The function body executes (it may read `env` and `size`).
3. `cpu_instruction_cost()` is recorded.
4. Consecutive (size, cost) pairs are compared: the observed cost ratio is
   checked against the ratio the model predicts.

**Growth models:**

- **`linear`** — cost ∝ n.  Expected ratio = `size_{i+1} / size_i`.
- **`quadratic`** — cost ∝ n².  Expected ratio = `(size_{i+1} / size_i)²`.

If the absolute deviation `|observed/expected - 1|` exceeds `tolerance`, the
test panics with a diagnostic that lists the offending size, expected and
observed ratios, deviation, and all measurements.

**Limitations:**

- The body must not use `return`, `break`, or `continue` that would exit the
  measurement loop.
- A fresh `Env` is created per iteration — setup that must persist across sizes
  should be extracted outside the macro.
- Small base costs can mask the growth signal at tiny sizes; choose sizes where
  the measured work dominates.
- Only CPU cost is checked.

### Applying a budget attribute to an `impl` block

`#[budget_cpu_lt]`, `#[budget_mem_lt]`, `#[budget_write_bytes_lt]`,
`#[budget_read_bytes_lt]` and `#[budget_lt]` may be placed on an `impl` block.
The limit then applies to **every** `fn` in the block:

```rust
struct Contract;

#[budget_cpu_lt(1_000_000)]
impl Contract {
    fn deposit() {
        let env = Env::default();
        // ... asserted against 1_000_000 ...
    }

    fn withdraw() {
        let env = Env::default();
        // ... also asserted against 1_000_000 ...
    }

    // Per-method override: this method's own attribute wins; the block
    // limit does not also apply.
    #[budget_cpu_lt(4_000_000)]
    fn rebalance() {
        let env = Env::default();
    }
}
```

Semantics:

- **Every function is instrumented**, `pub` or not. "Twelve entry points, one
  ceiling" is the motivating case, and a thirteenth added later is asserted
  automatically rather than shipping unbudgeted.
- **A method with its own `#[budget_*]` attribute is governed by that**, not the
  block limit — it is never asserted twice. This is also the opt-out: a helper
  that should not be budgeted can carry `#[budget_cpu_lt(env = "UNSET")]`, whose
  unset-env limit resolves to "no limit".
- **A failure names the method** — the panic message carries `` [fn `name`] ``.
- Every method must have a local `env` in scope, exactly as for the
  single-function form. A pure helper with no `env` belongs outside the block.
- `#[budget_scaling]` is **not** supported on `impl` blocks (it rewrites a
  function into a `#[test]`), and neither are modules or traits — those still
  fail with a compile error.

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

### Limit sources and their precedence

Every budget macro accepts the limit as one of four forms. They do not stack —
a limit comes from exactly one source, decided by the keys you write:

| Form | Where the limit is read from | When |
|---|---|---|
| `N` (integer literal) | the attribute itself | macro expansion |
| `config = "KEY"` | `budget.json` in the process working directory | test runtime |
| `env = "VAR"` | the `VAR` process environment variable | test runtime |
| `env_file = "PATH", env = "VAR"` | the `VAR` key inside the `KEY=VALUE` file at `PATH` | test runtime |

`env_file` **overrides `env` for that one test**: with both keys present the
limit is read from the file, never from the process environment, so two tests
in the same suite can resolve the same logical limit from different files by
carrying different `env_file` paths. A test with no `env_file` is unaffected
and keeps reading `VAR` from the process environment.

**`env_file` path resolution.** A *literal* path (`env_file = "../limits.env"`)
must resolve to a file at macro-expansion time — checked against
`CARGO_MANIFEST_DIR`, the build's working directory, and a `budget-macros/`
fallback — or the build fails with an error naming the path. A *non-literal*
path (`env_file = SOME_CONST`) is assumed to be produced by the build and is
resolved at test runtime instead; a missing file then panics the test (it is
never treated as "no limit"). At runtime the file is re-read per assertion, so
the mechanism is thread-safe and needs no `unsafe std::env::set_var`.

The checked-in `tier-a-limits.env` file is the recommended source for
Tier A limits. Its provenance, refresh procedure, and staleness-detection
guidance live in [`tier-a-limits.provenance.md`](../../tier-a-limits.provenance.md).

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

This section is the **complete** flag reference: every `#[arg(...)]` field declared on `BudgetReportArgs` in [`cargo-budget-report/src/cli.rs`][cli-rs] appears in the table below. [`scripts/check-cli-docs.sh`](#keeping-this-page-current) enforces that a newly added flag cannot be merged without at least a mention here, so the table cannot silently fall as far behind as it once had.

[cli-rs]: https://github.com/Tollcraft/soroban-budget-assert/blob/main/cargo-budget-report/src/cli.rs

### Full flag table

| Flag | `budget.toml` equivalent | Default | Purpose |
|---|---|---|---|
| `--network <NETWORK>` | `network` | none — required from one source | Network to deploy and invoke against, e.g. `testnet` (passed straight through to the `stellar` CLI). CLI flag wins over the file; missing from both is a fatal error naming the field. **Does not** actually change what the simulate step targets — see [the discrepancy note](#--network-does-not-actually-route-the-simulate-step) below. |
| `--source <SOURCE>` | `source` | none — required from one source | Funded Stellar identity used for deploy fees and as the simulation source. Same precedence as `--network`. |
| `--json` | — | `false` | Emit the report as pretty-printed JSON instead of a table. Composes with `--check` (adds `limit`/`pass` per entry) and with `--record-baseline`/`--check-baseline` (see [Output-format precedence](#output-format-precedence-when-flags-combine)). |
| `--csv` | — | `false` | Emit the report as CSV instead of a table. Header is `package,function,metric,value` normally, or `package,function,metric,value,limit,pass` under `--check`. Rows whose `value` never simulated are only included in `--check` mode (they carry `pass=false`); in the non-`--check` CSV they are omitted entirely, unlike the JSON/table output, which lists them. Takes priority over `--json`/`--html` if more than one is passed — see [below](#output-format-precedence-when-flags-combine). |
| `--html` | — | `false` | Emit the report as a single self-contained HTML page — no external CSS, scripts, or fonts, so it renders from a `file://` URL and from a downloaded CI artifact. Rows mirror the JSON output; with `--check` each row also shows its limit and pass/fail status. |
| `--markdown` | — | `false` | Emit the report as a GitHub-Flavored Markdown table suitable for appending to `$GITHUB_STEP_SUMMARY`. Numeric values are comma-formatted; unavailable metrics (e.g. network-only Read/Write Bytes) render as `N/A (testnet required)`. When used with `--from <PATH>`, reads an existing JSON report file instead of running a live simulation — this is the mode the CI workflow uses to render the step summary from `current_report.json`. |
| `--check` | — | `false` | Compare measured metrics against `cpu_limit` / `read_limit` / `write_limit` declared per function in `budget.toml`; print a per-function+metric pass/fail line and exit non-zero on any breach or failed configured simulation. See [`--check`: enforcing regression limits](#--check-enforcing-regression-limits-against-network-verified-costs). |
| `--color <auto\|always\|never>` | — | `auto` | When to colourise the plain-text `--check` report. Only meaningful together with `--check` — there is nothing to colourise otherwise, and callers gate on `args.check` before consulting it. See [the discrepancy note](#-color-does-not-actually-force-colour-into-a-pipe) below: `--color always` does **not**, despite its help text, force colour into a non-terminal output. |
| `--quiet` | — | `false` | Suppress non-essential progress messages and warnings on stderr (build/deploy/simulate progress, retry notices). The final report is still printed to stdout; fatal errors (spawn failures, hard build failures) still go to stderr regardless. |
| `--validate` | — | `false` | Re-decode each successful simulation's `SorobanTransactionData` XDR through `stellar xdr decode` and diff the result against the values this tool computed. Any discrepancy is reported as a diagnostic and the process exits non-zero. Silently **skipped** (not failed) when the Stellar CLI or its `xdr decode` subcommand is unavailable — this is a self-check against a second decoder, not a new data source. |
| `--profile <PROFILE>` | — | `release` | Cargo build profile used to compile each contract's WASM (`cargo build --profile <PROFILE>`). A custom profile (e.g. `release-opt`) must already be defined in the workspace `Cargo.toml`; the tool does not validate that it exists before invoking `cargo build` with it. |
| `--init` | — | `false` | Scaffold a commented `budget.toml` template at `./budget.toml` and exit immediately — no build, deploy, or simulation happens. Fails if `budget.toml` already exists unless `--force` is also passed. |
| `--force` | — | `false` | Only meaningful with `--init`: allows overwriting an existing `budget.toml`. Ignored (has no effect on anything) when `--init` is not also passed. |
| `--record-baseline <PATH>` | — | none | Write a new resource-usage baseline snapshot to `PATH` (conventionally `budget-baseline.toml`) and exit, instead of printing a report. Requires an explicit path argument — `--record-baseline` with no value is a clap parse error, not an implicit default filename. See [Step 6 of the End-User Guide](user_guide.md#step-6-optional-catch-regressions-on-the-workspace-with-a-baseline). |
| `--check-baseline <PATH>` | — | none | Check current measurements against the baseline snapshot at `PATH`, applying the configured regression tolerance (`--tolerance` / `tolerance` / per-function override). Exits non-zero on any regression beyond tolerance. Mutually exclusive in effect with `--record-baseline` — passing both resolves to whichever `Mode` is checked first in `Mode::from_args` (record wins); do not rely on that ordering, pass only one. |
| `--tolerance <F>` | `tolerance` (top-level) and `[functions.<name>].tolerance` (per-function) | `0.10` | Regression tolerance for `--check-baseline`, as a fraction (`0.10`) or a percentage (`"10%"`). CLI flag overrides the file's top-level `tolerance` — **except** a function's own `[functions.<name>].tolerance`, which outranks even this flag for that function. See [Value precedence](#value-precedence). |
| `--max-retry-attempts <N>` | `[retry].max_attempts` | `4` | Total attempts (including the first) for deploy, invoke-build, and simulate-RPC calls before giving up. `1` disables retry entirely; `0` is rejected with an error. See [`retry`: transient-failure retry policy](#retry-transient-failure-retry-policy) and the [testnet troubleshooting guide](testnet_troubleshooting.md) for what actually gets retried. |
| `--retry-backoff-secs <SECS>` | `[retry].initial_backoff_secs` | `2` | Initial backoff before the first retry; doubles on each subsequent attempt (2 → 4 → 8 with the defaults). |
| `--derive-limits <OUT>` | — | none | Derive local (Tier A) test limits from a Tier B JSON report and write them as `KEY=VALUE` pairs to `OUT`, then exit — no build/deploy/simulate happens in this mode. Reads the Tier B report from `--from` (or stdin). Requires either all four `--margin-*` flags or a complete `[margin]` block in `budget.toml`; see [`margin`: deriving Tier A limits](#margin-deriving-tier-a-limits). |
| `--from <PATH>` | — | stdin (`-`) | Source Tier B JSON report for `--derive-limits`. `-` (the default when omitted) reads from stdin, so `cargo budget-report --json \| cargo budget-report --derive-limits tier-a-limits.env --margin-cpu 1.5 ...` composes as a pipeline. Ignored outside `--derive-limits` mode. |
| `--margin-cpu <F>` | `[margin].cpu_margin` | none | Multiplier applied to Tier B CPU values when deriving Tier A limits. Must be finite and `>= 1.0`. |
| `--margin-memory <F>` | `[margin].memory_margin` | none | Multiplier applied to Tier B memory values. Same validity rule as `--margin-cpu`. |
| `--margin-read <F>` | `[margin].read_margin` | none | Multiplier applied to Tier B read-bytes values. Same validity rule as `--margin-cpu`. |
| `--margin-write <F>` | `[margin].write_margin` | none | Multiplier applied to Tier B write-bytes values. Same validity rule as `--margin-cpu`. All four `--margin-*` flags are all-or-nothing: supplying some but not all is an error listing the missing ones, and there is never a mix of CLI flags and a `[margin]` block — see [Value precedence](#value-precedence). |
| `--provenance-out <PATH>` | — | `<OUT>` with `.env` replaced by `.md` | Only meaningful with `--derive-limits`: where to write the Markdown provenance table documenting how each derived limit was computed. Defaults from `--derive-limits`'s own `OUT` path (e.g. `tier-a-limits.env` → `tier-a-limits.provenance.md`), so it rarely needs to be set explicitly. |
| `--record <PATH>` | — | none | Record every transport response (deploy, invoke-build, simulate RPC) into a replayable fixture file at `PATH`. The run itself still talks to the network; the fixture lets a later `--replay` run reproduce the same report offline. Mutually exclusive with `--replay` (rejected by clap's `conflicts_with` at parse time, before any network call happens). |
| `--replay <PATH>` | — | none | Replay a run from a fixture file written by `--record`. The whole report pipeline runs offline: no `stellar` CLI, no `curl`, no network access, and preflight checks for those tools are skipped entirely. Mutually exclusive with `--record`. |
| `--watch` | — | `false` | Watch the workspace for file changes and re-measure on save. Refuses to start when stdout is not a terminal. |

### Flags that interact

- **`--force` only does anything with `--init`.** Passing `--force` alone (no `--init`) is accepted by the parser but has no effect — it isn't read anywhere outside `scaffold_init`.
- **`--csv` / `--json` / `--html` / `--markdown` are mutually exclusive in effect, not by `conflicts_with`.** clap does not reject combining them; the renderer picks one output in a fixed priority order (`--csv` first, then `--json`, then `--markdown`, then `--html`, then the plain-text table). See [Output-format precedence](#output-format-precedence-when-flags-combine).
- **`--record` and `--replay` *are* enforced as mutually exclusive** via clap's `conflicts_with`, so passing both is a parse-time error naming both flags — unlike the `--csv`/`--json`/`--html` case above.
- **`--derive-limits` changes what every other network/build flag means.** In derive mode the tool never builds, deploys, or simulates anything; `--network`, `--source`, `--profile`, `--record`, `--replay`, and the retry flags are all irrelevant to that run. Only `--from`, `--margin-*`, and `--provenance-out` matter.
- **`--record-baseline` / `--check-baseline` also short-circuit the legacy report path**, similarly to `--derive-limits`: `--json` still applies (it selects JSON vs. text rendering of the *baseline* report), but `--csv`, `--html`, and `--check` do not apply in these modes.
- **`--tolerance` is overridden, not overriding, in one specific case**: a function's own `[functions.<name>].tolerance` in `budget.toml` wins even over an explicit `--tolerance` flag. Every other file-vs-flag precedence in this tool goes the other way (flag wins). See [Value precedence](#value-precedence).
- **`--max-retry-attempts` / `--retry-backoff-secs`** apply identically whether or not `--record-baseline`/`--check-baseline`/`--derive-limits` are active, because they gate the same underlying deploy/invoke/simulate calls those modes still make (except `--derive-limits`, which makes none).

### `budget.toml` fields vs. CLI flags: which one wins

Every flag in the table above that has a `budget.toml` equivalent column entry follows the same rule unless noted: **the CLI flag wins when both are present.** The one documented exception is per-function `tolerance` (see above). The full precedence table, including the margin all-or-nothing rule, lives at [Value precedence](#value-precedence) later in this page — it is not repeated per-flag here to avoid two sources of truth drifting apart.

### `budget.toml` schema validation

`budget.toml` is validated against the schema the tool understands **before any
report is produced** (in Report, Record, and Check modes). Validation fails
loudly instead of silently ignoring mistakes — the damaging case being a
misspelled function name, which previously yielded a report that simply omitted
the function with no indication anything was wrong (issue #399).

Every problem found is reported at once, so a misconfigured file takes one
round trip to fix rather than five. The error classes are:

- **Unknown top-level key** — a key that is a plausible typo of a known key is
  rejected with the key name, its location, and the closest valid key as a
  suggestion. For example, `tolernce = 0.1` reports
  `unknown top-level key \`tolernce\` (did you mean \`tolerance\`?)`. Arbitrary
  foreign sections — such as `[lints]`, consumed by the sibling
  `soroban-cost-linter` tool — are silently accepted so a single shared
  `budget.toml` can serve multiple tools without errors.
- **Type error** — names the offending field and the type that was expected
  (e.g. `cpu_limit = "high"` fails because `cpu_limit` must be a `u64`).
- **Misspelled limit key** — `FunctionConfig` denies unknown fields, so a typo
  such as `cpu_lmit = 5_000_000` is reported as a schema error naming the field.
- **Configured function does not exist** — a `[functions.<name>]` whose name is
  not an exported function of the workspace is reported as an error that lists
  the available functions (with a closest-match suggestion), e.g.
  `function \`do_expensive_wrk\` is configured in budget.toml but does not exist
  in the workspace (did you mean \`do_expensive_work\`?). Available functions: ...`.

The known top-level keys are `network`, `source`, `tolerance`, `margin`,
`scenarios`, `functions`, and `retry`.

### Output-format precedence when flags combine

`--csv`, `--json`, and `--html` are not declared as mutually exclusive to clap (`--record`/`--replay` are, via `conflicts_with`; these three are not). Passing more than one is accepted, and the renderer picks exactly one output in this fixed order, checked top to bottom in the source:

1. `--csv` (if set, nothing else is rendered)
2. `--json` (if set and `--csv` was not)
3. `--html` (if set and neither of the above was)
4. plain-text table (the fallback when none of the three are set)

So `cargo budget-report --csv --json` prints CSV only; `--json --html` prints JSON only. This is undocumented in the flags' own help text — verified by reading the rendering branch in `main.rs` rather than assumed.

### `--color` does not actually force colour into a pipe

`--color`'s own doc comment in `cli.rs` says `Always` will "always emit colour, even into pipes and files." That is not what the implementation does: `color_enabled_with` (the pure decision function backing `--color`, exhaustively unit-tested in `main.rs`) returns `false` whenever stdout is not a terminal or `NO_COLOR` is set, **before** it even looks at whether the choice was `Always`, `Auto`, or `Never`. A test in the same module asserts this directly: `--color always` piped to a file or another process produces no ANSI escapes. In practice `--always` and `--auto` currently behave identically; only `--never` is distinguishable from the other two. This looks like an intentional safety choice (never corrupt a file or a downstream parser with escape codes) that the help text's wording never caught up to — the behavior was not changed here, since changing flag behavior is out of scope for this page; only the discrepancy is reported.

### `--network` does not actually route the simulate step

`--network` selects the network for the `stellar contract deploy` and `stellar contract invoke --build-only` steps — those shell out to the `stellar` CLI with `--network <value>`, which resolves the name through the CLI's own network configuration correctly for `testnet`, `futurenet`, `local`, or any custom network. The final `simulateTransaction` RPC call does **not** go through the `stellar` CLI at all: `LiveTransport::simulate_transaction` in `live.rs` POSTs directly to a hardcoded `https://soroban-testnet.stellar.org:443`, regardless of what `--network` was set to. In practice this means:

- `--network testnet` (the common case, and the only one this project's own CI and examples use) behaves as documented — deploy, invoke, and simulate all target the same network.
- `--network futurenet`, `--network local`, or any other network deploys and builds the invocation correctly, but then simulates against testnet's RPC — which does not have the contract this run just deployed. Expect a simulation failure (see [Simulation failure](testnet_troubleshooting.md#simulation-failure-transaction-simulation-failed-or-similar)) that has nothing to do with the contract itself.

This is a real functional gap, not just missing prose — no flag or `budget.toml` field currently changes which RPC endpoint `simulateTransaction` targets. It is reported here rather than fixed, since changing flag behavior is out of scope for this page.

### Keeping this page current

The real failure mode here is drift, not the one-time gap this page used to have: a flag added to `cli.rs` in a future PR with no corresponding row here. [`scripts/check-cli-docs.sh`](https://github.com/Tollcraft/soroban-budget-assert/blob/main/scripts/check-cli-docs.sh) is a CI-enforced drift check (wired into `quality.yml`) that derives every `--kebab-case` flag name from `cli.rs`'s `#[arg(...)]`-decorated fields and fails the build if any of them is not at least mentioned somewhere in this file. It catches a flag being completely undocumented; it cannot catch prose that is present but wrong, incomplete, or stale relative to the flag's actual behavior — that still needs human review, ideally by running the flag rather than trusting its `--help` text (see the `--color` and `--csv`/`--json`/`--html` findings above, both of which the flags' own help text does not mention).
| Flag | Required | Meaning |
|---|---|---|
| `--network` | yes (flag or `budget.toml`) | Network to deploy and simulate against, e.g. `testnet` |
| `--source` | yes (flag or `budget.toml`) | Funded identity used for deploy fees and as the simulation source |
| `--json` | no | Emit the report as pretty-printed JSON instead of a table |
| `--html` | no | Emit the report as a single self-contained HTML page — no external CSS, scripts, or fonts, so it renders from a `file://` URL and from a downloaded CI artifact. Rows mirror the JSON output; with `--check` each row also shows its limit and pass/fail status |
| `--check` | no | Compare measured metrics against `cpu_limit` / `read_limit` / `write_limit` declared per function in `budget.toml`; print a per-function+metric pass/fail line and exit non-zero on any breach or failed configured simulation |
| `--record <PATH>` | no | Record every transport response (deploy, invoke-build, simulate RPC) into a replayable fixture file at `PATH`. The run itself still talks to the network; the fixture lets a later `--replay` reproduce the same report offline. Mutually exclusive with `--replay` |
| `--replay <PATH>` | no | Replay a run from a fixture written by `--record`. The whole pipeline runs offline — no `stellar` CLI, no `curl`, no network access — and the report is byte-identical to the recorded run. Mutually exclusive with `--record` |

Configuration precedence: a CLI flag overrides the `budget.toml` value. If neither provides `network`/`source`, the command exits with an error naming the missing field.

External requirements: the `stellar` CLI on `PATH`, a funded source identity on the target network, and the `wasm32-unknown-unknown` Rust target installed. `--replay` is the exception — it needs none of the network tooling (no `stellar`, no `curl`), only the workspace itself.

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

# Retry policy for network calls (deploy, invoke-build, simulate RPC).
[retry]
# Total attempts including the first. `1` disables retry entirely.
max_attempts = 4
# Seconds to wait before the first retry; doubles on each further attempt.
initial_backoff_secs = 2
```
{% endcode %}

- `network`, `source` — defaults for the corresponding CLI flags.
- `[functions.<name>].args` — arguments injected when simulating that exported function. Functions without an entry are simulated with no arguments; if a required argument is missing, the simulation fails with a warning and that function is skipped.
- `[functions.<name>].cpu_limit`, `.read_limit`, `.write_limit` — inclusive upper bounds for simulated CPU instructions, read bytes, and write bytes. Enforced only when `--check` is passed. A missing field means "not enforced" for that metric.

### `[retry]`: transient-failure retry policy

Deploy, invoke-build, and the simulate RPC request are all retried on *plausibly transient* failures: rate-limit responses (HTTP 429), connection errors and timeouts, and server-side blips (502/503). Deterministic failures — a contract that does not exist, a malformed XDR, an RPC-reported simulation error — are **not** retried, because repeating them cannot change the outcome.

| Field | CLI override | Default | Meaning |
|---|---|---|---|
| `max_attempts` | `--max-retry-attempts` | `4` | Total attempts per call site, including the first. `1` disables retry. |
| `initial_backoff_secs` | `--retry-backoff-secs` | `2` | Delay before the first retry; doubles on each subsequent attempt. |

Precedence is CLI over `budget.toml` over defaults, matching every other configurable key.

The worst-case time spent sleeping per call site is bounded and derivable from the config alone:

```text
initial_backoff_secs × (2^(max_attempts − 1) − 1)
```

With the defaults that is 2 + 4 + 8 = **14 s** per call site. A CI job with a tight time limit can set `max_attempts = 2` (worst case 2 s) or `max_attempts = 1` (no retry); against a private network that never rate-limits, `max_attempts = 1` removes the dead time entirely.

Retry progress messages go to stderr and are suppressed by `--quiet`.

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

### HTML output (`--html`)

`--html` emits the same data as `--json` as a single self-contained HTML file:

- One row per measured metric with the same names and numbers as the JSON output for the same run.
- Values are displayed with thousands separators; the raw number is kept in a `data-value` attribute on each cell so it can still be copied or scripted over.
- In `--check` mode each row gains a `Limit` column and a `Status` column. Pass/fail is conveyed with `✓ PASS` / `✗ FAIL` text and a glyph, so it is readable without relying on colour.
- Package and function names (which come from the workspace) are HTML-escaped before being placed in the page.
- A run with zero successful measurements produces a valid page with an explicit empty state rather than an empty file.

There are no external CSS files, scripts, or fonts — the page works from a `file://` URL and from a downloaded CI artifact. To share a report with someone who does not run `cargo`, pipe it to a file:

```bash
cargo budget-report --html > budget-report.html
```

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

## `budget.toml` schema reference

This is the complete reference for the `budget.toml` file. It was verified against the parser — the `BudgetToml`, `MarginToml`, `ScenarioToml`, `FunctionConfig`, and `RetryToml` types in `cargo-budget-report/src/main.rs` (with the core `BudgetToml`/`FunctionConfig`/`limit_for_metric` subset mirrored in `budget-core/src/lib.rs`) — not against the example file or the `--init` template, so where an example elsewhere in this repository is loose, this section is normative.

### File discovery and error handling

- The file is read from the path `budget.toml` **relative to the current working directory**. There is no ancestor-directory search in the parser: run the command from the workspace root, or pass everything via flags.
- A **missing**, empty, whitespace-only, or comments-only file is not an error — the tool proceeds with an all-default configuration (every section optional, nothing enforced).
- A file that exists but fails to parse aborts the run with a TOML error that includes line and column information.

### Top-level keys

| Key | Type | Required | Default | Effect |
|---|---|---|---|---|
| `network` | string | no | none | Target network for deploy/simulate (`"testnet"`, `"futurenet"`, `"local"`, or a custom network from your Stellar CLI config). Falls back to `--network`; if neither is set the run aborts with `missing --network or budget.toml network field`. |
| `source` | string | no | none | Stellar source identity used for deployment fees and as simulation source. Falls back to `--source`; if neither is set the run aborts. |
| `tolerance` | number (fraction) | no | `0.10` | Default regression tolerance for `--check-baseline`. Overridable per function (see below) and by `--tolerance`. |
| `[margin]` | table | no | none | Per-metric margin multipliers consumed only by `--derive-limits`. See [below](#margin-deriving-tier-a-limits). |
| `[scenarios.<name>]` | table of tables | no | none | Function-to-scenario mapping consumed only by `--derive-limits`. See [below](#scenariosnamemapping-functions-to-derived-scenario-limits). |
| `[functions.<name>]` | table of tables | no | none | Per-function configuration. See [below](#functionsnameper-function-configuration). |
| `[retry]` | table | no | built-in defaults | Retry policy for deploy / invoke-build / simulate RPC calls. See [below](#retrytransient-failure-retry-policy). |

**Unknown top-level keys and sections are silently accepted.** This is deliberate: `[lints]` (for soroban-cost-linter) and other foreign sections let two tools share one `budget.toml`. Note the asymmetry with `[functions.*]`, where unknown keys are a hard error.

### Value precedence

Where a value can come from more than one place, this is the exact order the resolver applies:

| Value | 1st choice | 2nd choice | Fallback |
|---|---|---|---|
| Network | `--network` flag | `network` key | fatal error naming the field |
| Source identity | `--source` flag | `source` key | fatal error naming the field |
| Baseline tolerance | per-function `tolerance`¹ | `--tolerance` flag | `tolerance` key, then `0.10` |
| Derive margins | all four `--margin-*` flags² | complete `[margin]` block | fatal error ("no margin supplied") |
| Retry policy | `--max-retry-attempts` / `--retry-backoff-secs` | `[retry]` block | defaults (4 attempts, 2 s) |

1. The per-function override is the one place where file configuration outranks a command-line flag: during `--check-baseline`, a function's own `tolerance` beats even `--tolerance`.
2. Margin flags are all-or-nothing: supplying any subset of the four flags is an error listing the missing ones; there is never a mix of CLI and file margins.

`cargo budget-report` reads **no environment variables** as configuration input. Environment variables appear on the output side only: `--derive-limits` writes `KEY=VALUE` pairs into `tier-a-limits.env` for the Tier A test macros to consume at test time.

### `functions.<name>`: per-function configuration

The section key `<name>` must match the **exported WASM function name exactly** (case-sensitive). Names are not package-qualified: if two contracts export the same function name, the single entry applies to both simulations.

| Field | Type | Required | Default | Effect |
|---|---|---|---|---|
| `args` | array of strings | no | `[]` | Forwarded verbatim after the `--` separator to `stellar contract invoke -- <fn> <args>`. Functions without an entry are simulated with no arguments. |
| `cpu_limit` | integer (u64) | no | none | Inclusive upper bound on the measured `CPU Instructions` metric in `--check` mode. |
| `read_limit` | integer (u64) | no | none | Inclusive upper bound on `Read Bytes`. |
| `write_limit` | integer (u64) | no | none | Inclusive upper bound on `Write Bytes`. |
| `tolerance` | number (fraction) | no | global tolerance | Per-function regression-tolerance override applied during `--check-baseline`. Takes precedence over `--tolerance`. |

- A missing `*_limit` field means that metric is **reported but not enforced**.
- Limits are inclusive: a measurement equal to the limit passes.
- There is deliberately no limit field for `WASM Bytes` — binary size is reported but can never be enforced through this file.
- **Unknown keys inside a `[functions.*]` block produce a parse error** (e.g. a typo like `cpu_lmit` fails the run instead of silently doing nothing).
- Under `--check`, a configured function whose *simulation fails* counts as a check failure even when none of its limits are set, so a broken invocation cannot masquerade as a pass.

{% hint style="warning" %}
A `[functions.<name>]` entry whose name does not match any exported function of any workspace package is **silently ignored** — no warning, no error, nothing measured. This is the file's main trap: a typo in a function name looks identical to a function you chose not to configure. Cross-check names against the export list (e.g. `stellar contract invoke --help` output or a passing report row) when limits mysteriously fail to apply.
{% endhint %}

### What happens when a listed function or package does not exist

Simulation is driven by what the workspace *exports*, not by what the file declares:

1. Every workspace package with a `cdylib` target is built for `wasm32` and deployed.
2. Every exported function (excluding names starting with `_` and the `memory` export) is simulated — with its `[functions.*]` entry if one exists, without arguments otherwise.
3. Config entries are then looked up by name. Consequences:
   - An entry for a function absent from every package's exports is **ignored silently** (see the warning above).
   - A package cannot be "listed" in `budget.toml` at all — there is no per-package section. Packages are discovered from `cargo metadata`, and removing or renaming a package needs no config change; renaming an exported *function*, however, orphans its entry silently.
   - Because entries are keyed by bare function name, one entry fans out to every package exporting that name.

### `margin`: deriving Tier A limits

Consumed only by `cargo budget-report --derive-limits`; ignored by every other mode.

| Field | Type | Required | Default | Effect |
|---|---|---|---|---|
| `cpu_margin` | number | see note | none | Multiplier applied to Tier B CPU values. |
| `memory_margin` | number | see note | none | Multiplier applied to Tier B memory values. |
| `read_margin` | number | see note | none | Multiplier applied to Tier B read-bytes values. |
| `write_margin` | number | see note | none | Multiplier applied to Tier B write-bytes values. |

Each field is individually optional *at parse time*, but the block is usable only when **complete**: if no `--margin-*` flags are given, an incomplete `[margin]` block produces the same `no margin supplied` error as no block at all. All four values must be finite and `>= 1.0`; a sub-1.0 margin would tighten the limit below the measured Tier B value and is rejected. No default is ever picked silently — margins are treated as audit-trail data.

For the current margin values, the Tier A limits they produce, and the protocol version the numbers correspond to, see [`tier-a-limits.provenance.md`](../../tier-a-limits.provenance.md).

### `scenarios.<name>`: mapping functions to derived scenario limits

Consumed only by `--derive-limits`. Each scenario sums the Tier B values of its component functions into a single Tier A limit under one environment-variable key. See [`tier-a-limits.provenance.md`](../../tier-a-limits.provenance.md) for the current derived limits and their refresh procedure.

| Field | Type | Required | Default | Effect |
|---|---|---|---|---|
| `package` | string | effectively yes | `""` | Package namespace of the scenario. The derived env-var key is `<package>::<name>`; omitting `package` yields `::<name>`, which will not line up with a test annotation. |
| `functions` | array of strings | no | `[]` | Exported function names whose Tier B values are summed into the scenario's Tier A limit. |

Syntax is `[scenarios.<name>]` — a table of tables — despite some doc comments spelling it `[[scenarios.<name>]]`.

### `retry`: transient-failure retry policy

Controls how many times deploy, invoke-build, and the simulate RPC call are retried on plausibly transient failures (rate limits, connection errors, 5xx blips). Deterministic errors (missing contract, malformed XDR, simulation errors) are never retried.

| Field | Type | Required | Default | CLI override | Effect |
|---|---|---|---|---|---|
| `max_attempts` | integer (u32) | no | `4` | `--max-retry-attempts` | Total attempts including the first. `1` disables retry; `0` is rejected. |
| `initial_backoff_secs` | integer (u64) | no | `2` | `--retry-backoff-secs` | Delay before the first retry; doubles each further attempt. |

Unlike `[margin]`, a partial `[retry]` block is fine: each missing field keeps its default independently. Worst-case sleep per call site is `initial_backoff_secs × (2^(max_attempts − 1) − 1)` — 14 s with the defaults.

### Worked example: minimal

```toml
network = "testnet"
source = "alice"
```

With just these two lines (or the equivalent `--network` / `--source` flags), every exported function of every cdylib workspace package is simulated with no arguments and reported; nothing is enforced and baseline comparisons use the default 10% tolerance. Omitting both keys is also valid if they are supplied as flags.

### Worked example: realistic multi-package setup

```toml
network = "testnet"
source = "alice"

# Regression gate for --check-baseline: allow up to 10% growth unless a
# function overrides it below.
tolerance = 0.10

# -- AMM pool ------------------------------------------------------------
[functions.initialize]
args = ["--admin", "alice", "--fee-bps", "30"]
cpu_limit = 5000000
write_limit = 1000

[functions.deposit]
args = ["--amount", "10000000"]
cpu_limit = 5000000
read_limit = 5000
write_limit = 1000
tolerance = 0.05        # hot path: tighter regression budget than the global 10%

[functions.swap]
args = ["--amount", "1000000", "--min-out", "900000"]
cpu_limit = 8000000
read_limit = 8000
write_limit = 1200

[functions.withdraw]
args = ["--amount", "10000000"]

# -- Synthetic baseline (exercises loops + instance storage) --------------
[functions.do_expensive_work]
args = ["--n", "10000"]
cpu_limit = 5000000
read_limit = 5000
write_limit = 1000

# -- Tier A derivation (only consulted by --derive-limits) -----------------
[margin]
cpu_margin    = 1.50   # 50% headroom over Tier B CPU ceiling
memory_margin = 1.25   # 25% headroom over Tier B memory ceiling
read_margin   = 2.00   # 100% headroom over Tier B read-bytes ceiling
write_margin  = 3.00   # 200% headroom over Tier B write-bytes ceiling

# deposit + swap + withdraw summed into one Tier A limit for tests that
# exercise the whole workflow in a single assertion. The derived env var
# will be named amm-pool-contract::full_workflow.
[scenarios.full_workflow]
package = "amm-pool-contract"
functions = ["deposit", "swap", "withdraw"]

# -- Network retry policy ---------------------------------------------------
[retry]
max_attempts = 4
initial_backoff_secs = 2

# -- Foreign section: consumed by soroban-cost-linter, ignored here ---------
[lints]
complexity = "warn"
```

Effects of this file:

- `withdraw` has an entry but no `*_limit` fields, so its metrics are reported and it participates in `--check`'s fail-on-simulation-error rule, but no metric is enforced.
- If `amm-pool-contract` were renamed tomorrow, every section above would keep working unchanged except `[scenarios.full_workflow]`, whose `package` value must match the annotation used by Tier A tests.
- If `initialize` had been misspelled (e.g. `[functions.initialise]`), nothing would error and nothing would fail: the orphaned entry would be ignored, `initialize` would run unconfigured with no arguments, and none of its metrics would be enforced — the only symptom is missing rows in a later report.

### `#[budget_lt(N)]`

Generic budget assertion — asserts that **both** CPU instruction cost **and**
memory bytes cost are strictly less than `N`. This macro is a convenience
shorthand when you want a single ceiling covering both metrics.

**Static limit:**

```rust
use budget_macros::budget_lt;

#[test]
#[budget_lt(1_000_000)]
fn test_generic_budget() {
    let env = Env::default();
    // ... contract invocation ...
}
```

**Dynamic limit from environment variable:**

```rust
#[test]
#[budget_lt(env = "GENERIC_BUDGET_LIMIT")]
fn test_generic_with_env() {
    std::env::set_var("GENERIC_BUDGET_LIMIT", "1000000");
    let env = Env::default();
    // ...
}
```

**Limit from `.env` file:**

```rust
#[test]
#[budget_lt(env_file = "../tier-a-limits.env", env = "TIER_A__GENERIC__LIMIT")]
fn test_generic_from_file() {
    let env = Env::default();
    // ...
}
```

**Config-driven limit:**

```rust
#[test]
#[budget_lt(config = "generic_budget")]
fn test_generic_from_config() {
    let env = Env::default();
    // ...
}
```

**Baseline subtraction:**

```rust
#[test]
#[budget_lt(1_000_000, baseline = instantiation_floor())]
fn test_generic_marginal() {
    let env = Env::default();
    // ...
}
```

Failure message format:
```
CPU instruction cost {cpu} exceeded limit {N}
Memory bytes cost {mem} exceeded limit {N}
```

The first check that fails triggers the panic — if both are over, only the
CPU failure is reported. The macro supports the same attribute forms as
`budget_cpu_lt` and `budget_mem_lt`: integer literal, `env = "VAR"`,
`env_file = "PATH"`, `config = "key"`, and `baseline = <expr>`.


### `#[budget_write_bytes_lt(N)]` — write-bytes assertion

Asserts that the ledger write bytes used by `env` are strictly less than `N`.
Write bytes represent the total bytes written to ledger storage during contract
execution. This macro measures the local `memory_bytes_cost` as a proxy, which
correlates with storage serialization overhead even though the exact on-network
write-bytes figure is only available via RPC simulation.

**Static limit:**

```rust
use budget_macros::budget_write_bytes_lt;

#[test]
#[budget_write_bytes_lt(4096)]
fn test_write_bytes_budget() {
    let env = Env::default();
    // ... register contract as WASM, invoke client ...
}
```

**Dynamic limit from environment variable:**

```rust
#[test]
#[budget_write_bytes_lt(env = "MAX_WRITE_BYTES")]
fn test_write_bytes_with_env() {
    std::env::set_var("MAX_WRITE_BYTES", "4096");
    let env = Env::default();
    // ...
}
```

**Limit from `.env` file:**

```rust
#[test]
#[budget_write_bytes_lt(
    env_file = "../tier-a-limits.env",
    env = "TIER_A__AMM_POOL_CONTRACT__DEPOSIT__WRITE"
)]
fn test_write_bytes_from_file() {
    let env = Env::default();
    // ...
}
```

**Config-driven limit from `budget.json`:**

```rust
#[test]
#[budget_write_bytes_lt(config = "write_bytes")]
fn test_write_bytes_from_config() {
    let env = Env::default();
    // ...
}
```

**Baseline subtraction** — `baseline = <expr>` subtracts a fixed floor from the
measurement before comparison, so the *marginal* write-bytes cost is asserted.
The subtraction saturates at 0.

```rust
#[test]
#[budget_write_bytes_lt(4096, baseline = instantiation_floor_write_bytes())]
fn test_marginal_write_bytes() {
    let env = Env::default();
    // ...
}
```

Failure message format:
```
Write bytes cost (memory proxy) {actual} exceeded limit {N} - local estimate, underestimates real network cost
```
and with a baseline:
```
Write bytes cost (memory proxy) {marginal} exceeded limit {N} (marginal: {measured} measured - {baseline} baseline) - local estimate, underestimates real network cost
```

### `#[budget_read_bytes_lt(N)]` — read-bytes assertion

Asserts that the ledger read bytes used by `env` are strictly less than `N`.
Read bytes represent the total bytes read from ledger storage during contract
execution. This macro measures the local `memory_bytes_cost` as a proxy, which
correlates with storage access overhead even though the exact on-network
read-bytes figure is only available via RPC simulation.

**Static limit:**

```rust
use budget_macros::budget_read_bytes_lt;

#[test]
#[budget_read_bytes_lt(4096)]
fn test_read_bytes_budget() {
    let env = Env::default();
    // ...
}
```

**Dynamic limit:**

```rust
#[test]
#[budget_read_bytes_lt(env = "MAX_READ_BYTES")]
fn test_read_bytes_with_env_limit() {
    let env = Env::default();
    // ...
}
```

**Limit from a `.env` file:**

```rust
#[test]
#[budget_read_bytes_lt(
    env_file = "../tier-a-limits.env",
    env = "TIER_A__AMM_POOL_CONTRACT__DEPOSIT__READ"
)]
fn test_read_bytes_from_file() {
    let env = Env::default();
    // ...
}
```

**Config-driven limit:**

```rust
#[test]
#[budget_read_bytes_lt(config = "read_bytes")]
fn test_read_bytes_from_config() {
    let env = Env::default();
    // ...
}
```

**Baseline subtraction** — same semantics as `budget_write_bytes_lt`:

```rust
#[test]
#[budget_read_bytes_lt(4096, baseline = instantiation_floor_read_bytes())]
fn test_marginal_read_bytes() {
    let env = Env::default();
    // ...
}
```

Failure message format:
```
Read bytes cost (memory proxy) {actual} exceeded limit {N} - local estimate, underestimates real network cost
```
and with a baseline:
```
Read bytes cost (memory proxy) {marginal} exceeded limit {N} (marginal: {measured} measured - {baseline} baseline) - local estimate, underestimates real network cost
```

### Complete macro quick-reference

| Macro | Measures | Failure prefix | Baseline support |
|---|---|---|---|
| `#[budget_cpu_lt(N)]` | CPU instructions | `CPU instruction cost` | Yes |
| `#[budget_mem_lt(N)]` | Memory bytes | `Memory bytes cost` | Yes |
| `#[budget_write_bytes_lt(N)]` | Write bytes (memory proxy) | `Write bytes cost (memory proxy)` | Yes |
| `#[budget_read_bytes_lt(N)]` | Read bytes (memory proxy) | `Read bytes cost (memory proxy)` | Yes |
| `#[budget_lt(N)]` | Both CPU + memory | `CPU instruction cost` or `Memory bytes cost` | Yes |
| `#[budget_scaling(...)]` | CPU growth model | `{fn} scaling: {model}` | N/A |

All macros accept the same limit sources: integer literal, `env = "VAR"`,
`env_file = "PATH", env = "VAR"`, or `config = "key"`. The `baseline` parameter
is available on all macros except `budget_scaling`.
