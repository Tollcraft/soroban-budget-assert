# Adding a New Cost Metric

This guide walks through every place a new cost metric must be added across the
`soroban-budget-assert` workspace. It was written by tracing the **write bytes**
metric end-to-end — the cleanest, most recently added metric in the codebase —
so each step is grounded in the real implementation rather than inferred.

Use this guide when adding any new resource metric (read bytes, events, ledger
entries, or anything queued in open issues). Follow the checklist at the end to
make sure nothing is missed.

## Why a coordinated-edits guide exists

Every cost metric in this workspace touches four crates and one documentation
file. The edits are not independent: the macro generates code that calls a
specific budget method, the CLI reads a specific field from the XDR response,
the limit-resolution code maps a specific metric label to a specific config
field, and the derivation pipeline emits a specific env-var segment. If any
piece is missing, the metric silently produces zeros, panics at runtime, or
simply never appears in the report.

The sibling repository
[`soroban-cost-linter`](https://github.com/Tollcraft/soroban-cost-linter) uses a
similar coordinated-edits guide —
[`DEVELOPING_LINTS.md`](https://github.com/Tollcraft/soroban-cost-linter/blob/main/DEVELOPING_LINTS.md) —
to walk contributors through every registration point a new lint requires. This
guide mirrors that checklist style.

---

## Step 1 — Define the metric field in `budget-core`

**File:** `budget-core/src/lib.rs`

**What to do:** Add a new `Option<u64>` field to `FunctionConfig` and wire it
into `limit_for_metric()`.

`FunctionConfig` is the deserialized representation of a `[functions.<name>]`
section in `budget.toml`. Each metric needs a corresponding `*_limit` field so
that `--check` mode can enforce it:

```rust
pub struct FunctionConfig {
    pub args: Vec<String>,
    pub cpu_limit: Option<u64>,
    pub read_limit: Option<u64>,
    pub write_limit: Option<u64>,
    // Add:  pub your_new_limit: Option<u64>,
}
```

The `limit_for_metric()` function maps the human-readable metric label to the
config field. Add a new arm:

```rust
pub fn limit_for_metric(func_config: &FunctionConfig, metric: &str) -> Option<u64> {
    match metric {
        "CPU Instructions" => func_config.cpu_limit,
        "Read Bytes" => func_config.read_limit,
        "Write Bytes" => func_config.write_limit,
        // Add:  "Your New Metric" => func_config.your_new_limit,
        _ => None,
    }
}
```

**Why this step is required:** Without this field, the metric has no way to
carry a configured limit from `budget.toml` through to the check logic. The
`emit_check_failure_entries()` function in both `budget-core` and
`cargo-budget-report` iterates over the metric labels and looks up each one
through this function — an unknown label returns `None` and is silently skipped.

**Write-bytes trace:** The `write_limit: Option<u64>` field at line ~68 of
`budget-core/src/lib.rs` and the `"Write Bytes" => func_config.write_limit` arm
at line ~78 are the concrete write-bytes additions.

---

## Step 2 — Add the proc-macro attribute in `budget-macros`

**File:** `budget-macros/src/lib.rs`

**What to do:** Create a new `#[budget_<metric>_lt(N)]` proc-macro attribute.

Each metric gets its own attribute macro so the panic message and the measured
budget method are metric-specific. The macro must:

1. Parse `StandaloneSpec` (integer literal, `env = "VAR"`, `env_file` +
   `env`, `config = "KEY"`, `pct`).
2. Call `generate_limit_expr()` to build the limit expression from the parsed
   spec.
3. Choose the correct budget method to measure (e.g. `budget.memory_bytes_cost()`
   for write bytes, `budget.instructions()` for CPU).
4. Call `generate_metric_assert()` with the cost ident, the budget-method quote,
   the limit expression, and metric-specific panic messages.
5. Wrap the assertion in a block and pass it to `instrument_exit_paths()`.

For write bytes, the macro reads `budget.memory_bytes_cost()` as a local proxy
for on-network write-bytes (the exact figure is only available via RPC
simulation):

```rust
#[proc_macro_attribute]
pub fn budget_write_bytes_lt(attr: TokenStream, item: TokenStream) -> TokenStream {
    let spec = match syn::parse::<StandaloneSpec>(attr) { ... };
    let mut input_fn = match syn::parse::<ItemFn>(item) { ... };

    let limit_expr = generate_limit_expr(&spec.limit, "budget_write_bytes_lt");

    let env_ident = proc_macro2::Ident::new("env", ...);
    let cost_ident = proc_macro2::Ident::new("write_bytes_cost", ...);
    let assert_tokens = generate_metric_assert(
        &cost_ident,
        quote! { budget.memory_bytes_cost() },
        &limit_expr,
        spec.baseline.as_ref(),
        "Write bytes cost (memory proxy) {} exceeded limit {} ...",
        "Write bytes cost (memory proxy) {} exceeded limit {} (marginal: ...)",
    );

    // ... instrument_exit_paths wrapping ...
}
```

**Why this step is required:** The macro is the Tier A local-regression gate.
Without it, contributors have no compile-time/assertion-time check that a
contract's resource usage stays within bounds during `cargo test`.

**Write-bytes trace:** The `budget_write_bytes_lt` function starts at line ~1120
of `budget-macros/src/lib.rs`. It uses `budget.memory_bytes_cost()` as its proxy
and emits the `write_bytes_cost` ident.

---

## Step 3 — Wire the metric into `cargo-budget-report`

**File:** `cargo-budget-report/src/`

This crate is the Tier B reporting and Tier A limit-derivation tool. The new
metric must appear in several places:

### 3a. `FunctionConfig` in `main.rs`

Add a `write_limit` (or your metric's `*_limit`) field to the local
`FunctionConfig` struct (line ~387) and the `limit_for_metric()` match arm
(line ~588):

```rust
pub(crate) struct FunctionConfig {
    args: Vec<arg_spec::ArgSpec>,
    cpu_limit: Option<u64>,
    read_limit: Option<u64>,
    write_limit: Option<u64>,
    // Add:  your_new_limit: Option<u64>,
    tolerance: Option<f64>,
}
```

```rust
pub(crate) fn limit_for_metric(func_config: &FunctionConfig, metric: &str) -> Option<u64> {
    match metric {
        "CPU Instructions" => func_config.cpu_limit,
        "Read Bytes" => func_config.read_limit,
        "Write Bytes" => func_config.write_limit,
        // Add:  "Your New Metric" => func_config.your_new_limit,
        _ => None,
    }
}
```

### 3b. `Resources` struct in `main.rs`

Add the new field to the `Resources` struct (line ~350) that deserializes the
`simulateTransaction` RPC response:

```rust
pub(crate) struct Resources {
    instructions: u64,
    disk_read_bytes: u64,
    write_bytes: u64,
    // Add:  your_new_field: u64,
}
```

The field name must match the XDR field name in `SorobanTransactionData`. For
write bytes this is `write_bytes`; for read bytes the Protocol 23 XDR renamed it
to `disk_read_bytes`.

### 3c. `MeasuredResources` and `Measurement` in `main.rs` / `compare.rs`

Add the field to `MeasuredResources` (line ~397 in `main.rs`) and its
`as_compare()` conversion, and to `Measurement` in `compare.rs` (line ~31):

```rust
pub(crate) struct MeasuredResources {
    instructions: u64,
    read_bytes: u64,
    write_bytes: u64,
    // Add:  your_new_field: u64,
}
```

```rust
pub struct Measurement {
    pub cpu_instructions: u64,
    pub read_bytes: u64,
    pub write_bytes: u64,
    // Add:  pub your_new_field: u64,
}
```

The baseline TOML format in `compare.rs` also serializes/deserializes each
metric — add the field to `BaselineEntry` and the read/write logic.

### 3d. Report row creation in `main.rs`

In the simulation success arm (line ~1650), add the metric to the
`for (metric, value)` loop that builds `CostReport` entries:

```rust
for (metric, value) in [
    ("CPU Instructions", instructions),
    ("Read Bytes", read_bytes),
    ("Write Bytes", write_bytes),
    // Add:  ("Your New Metric", your_new_value),
    ("WASM Bytes", wasm_size),
] { ... }
```

### 3e. `emit_check_failure_entries()` in `main.rs`

Add the metric label to the loop (line ~628) that emits stub failure entries
when simulation fails:

```rust
for metric in ["CPU Instructions", "Read Bytes", "Write Bytes"] {
    // Add "Your New Metric" to this list
    let limit = limit_for_metric(func_config, metric);
    reports.push(CostReport { ... });
}
```

### 3f. `limit_checks.rs`

Add a check function and its tests:

```rust
pub fn check_write_bytes(bytes: u32, limit: u64) -> Result<(), String> {
    check_bounds(u64::from(bytes), limit, "Write Bytes")
}
```

### 3g. `derive.rs` — Limit derivation pipeline

The derivation pipeline (`--derive-limits`) needs the metric in three places:

1. **`Margin::for_metric()`** (line ~111): map the metric label to its margin
   multiplier:
   ```rust
   "Write Bytes" => Some(self.write),
   ```

2. **`metric_to_env_segment()`** (line ~522): map the metric label to its
   env-var key segment:
   ```rust
   "Write Bytes" => "WRITE",
   ```
   This produces keys like `TIER_A__AMM_POOL_CONTRACT__DO_EXPENSIVE_WORK__WRITE`.

3. **Scenario loop** (line ~342): add the metric label to the
   `for metric_label in [...]` list that sums Tier B values across scenario
   components.

### 3h. `validate.rs`

Add the new metric to `validate_metrics()` and `compare_metrics()` so the
`--validate` cross-check against the Stellar CLI includes it.

### 3i. `watch.rs`

Add the metric to the `metrics` array (line ~119) and the `get_metric()` match
so watch-mode diffing includes it:

```rust
let metrics = ["cpu_instructions", "read_bytes", "write_bytes"];
// Add "your_new_field" to this list
```

```rust
fn get_metric(m: &Measurement, metric: &str) -> Option<u64> {
    match metric {
        "cpu_instructions" => Some(m.cpu_instructions),
        "read_bytes" => Some(m.read_bytes),
        "write_bytes" => Some(m.write_bytes),
        // Add:  "your_new_field" => Some(m.your_new_field),
        _ => None,
    }
}
```

**Why all these sub-steps are required:** The CLI is the single source of truth
for Tier B measurements. If the metric is missing from any of these places, it
either won't be measured, won't appear in the report, won't have limits
derived, or won't be enforced in `--check` mode.

**Write-bytes trace:** `write_bytes` appears in `Resources` (line ~353),
`MeasuredResources` (line ~397), `Measurement` (line ~31 of `compare.rs`),
`limit_for_metric` (line ~588), `emit_check_failure_entries` (line ~628), the
report loop (line ~1665), `check_write_bytes` (line ~23 of `limit_checks.rs`),
`Margin::for_metric` (line ~111 of `derive.rs`), `metric_to_env_segment`
returning `"WRITE"` (line ~522), the scenario loop (line ~342), and
`get_metric` in `watch.rs` (line ~162).

---

## Step 4 — Document the metric in the measurements page

**File:** `docs/src/measurements.md`

Add a section to the measurements page recording:

- The operation type and fixture used.
- Local vs. network figures (when available).
- The delta calculation.
- The build profile and toolchain.

The write-bytes measurement lives in the "Memory bytes" section and uses the
`amm-pool-contract::write_bytes(1,024 bytes)` fixture with a size-optimized
WASM build. Its capture record is checked in at
`cargo-budget-report/fixtures/storage_write_benchmark.json`.

**Why this step is required:** The measurements page is the canonical record of
local-vs-network gaps. Without it, future contributors cannot assess whether a
metric's Tier A margins are still valid or need recalibration.

---

## Coordinated-edits checklist

Use this checklist to track every file that must change. Check each box as you
complete the corresponding edit. **All items must be checked before the PR is
ready.**

### `budget-core`
- [ ] Add `your_new_limit: Option<u64>` to `FunctionConfig`
- [ ] Add arm to `limit_for_metric()` mapping `"Your New Metric"` to
      `func_config.your_new_limit`
- [ ] Add `emit_check_failure_entries()` loop entry if the function iterates
      over the metric label list

### `budget-macros`
- [ ] Create `#[budget_your_new_lt(N)]` proc-macro attribute function
- [ ] Choose the correct `budget.*_cost()` method for the metric
- [ ] Choose metric-specific panic messages (with and without baseline)
- [ ] Wire through `instrument_exit_paths()`

### `cargo-budget-report`
- [ ] Add `your_new_limit` to `FunctionConfig` in `main.rs`
- [ ] Add arm to `limit_for_metric()` in `main.rs`
- [ ] Add field to `Resources` struct (must match XDR field name)
- [ ] Add field to `MeasuredResources` and `as_compare()` in `main.rs`
- [ ] Add field to `Measurement` in `compare.rs`
- [ ] Add field to `BaselineEntry` and baseline read/write logic in `compare.rs`
- [ ] Add `("Your New Metric", value)` to the report-row loop in `main.rs`
- [ ] Add metric label to `emit_check_failure_entries()` in `main.rs`
- [ ] Add `check_your_new_metric()` function and tests in `limit_checks.rs`
- [ ] Add `Margin::for_metric()` arm in `derive.rs`
- [ ] Add `metric_to_env_segment()` arm in `derive.rs` (e.g. `"WRITE"`)
- [ ] Add metric label to the scenario summation loop in `derive.rs`
- [ ] Add metric to `validate_metrics()` / `compare_metrics()` in `validate.rs`
- [ ] Add metric to `metrics` array and `get_metric()` in `watch.rs`

### Documentation
- [ ] Add measurement section to `docs/src/measurements.md`

### Test fixtures
- [ ] Add UI pass fixture in `budget-macros/tests/ui/pass/`
- [ ] Add unit tests for limit resolution in `budget-core/src/lib.rs`
- [ ] Add unit tests in `cargo-budget-report/src/limit_checks.rs`
- [ ] Add boundary tests in `cargo-budget-report/src/boundary_tests.rs`
- [ ] Add edge-case tests in `cargo-budget-report/src/edge_case_tests.rs`

---

## Test obligations

### Macro UI fixtures

Every `budget_*_lt` macro must have a UI pass fixture at
`budget-macros/tests/ui/pass/pass_<metric>.rs`. The fixture should test:

- A unit body (no return value).
- A trailing-expression body (returns a value).
- An early-return body (to verify `instrument_exit_paths` rewrites both exit
  paths).

For write bytes, the fixture is at `budget-macros/tests/ui/pass/pass_write_bytes.rs`
and exercises all three body shapes against a mock `Env` with known
`memory_bytes_cost()` values.

Run the UI tests with:

```bash
cargo test -p budget-macros
```

### Unit tests for limit resolution

`budget-core/src/lib.rs` has a `#[cfg(test)] mod tests` with a
`limit_for_metric_write_bytes` test that constructs a `FunctionConfig` with
`write_limit: Some(500)` and asserts `limit_for_metric(&config, "Write Bytes")`
returns `Some(500)`. Add an equivalent test for your metric.

`cargo-budget-report/src/edge_case_tests.rs` and
`cargo-budget-report/src/boundary_tests.rs` contain analogous tests for the
CLI's copy of `limit_for_metric` and `FunctionConfig`. Add equivalent coverage
there.

### Output-format coverage

`cargo-budget-report` renders metrics as text tables, JSON, CSV, and HTML. The
boundary and edge-case test files verify that:

- `format_with_commas_and_units()` renders byte metrics with a `B` suffix and
  instruction metrics with an `inst.` suffix.
- CSV output includes the metric label in the header.
- JSON output includes the metric field.

Add tests for your metric's rendering in the appropriate test files.

### Limit-check tests

`cargo-budget-report/src/limit_checks.rs` has tests like
`write_bytes_under_limit_passes` and `write_bytes_over_limit_fails`. Add
equivalent pass/fail tests for your metric's check function.

---

## How network limits are sourced

Tier A limits are **not** hardcoded. They are derived from Tier B
(network-simulated) measurements using the derivation pipeline in
`cargo-budget-report/src/derive.rs`.

### The derivation workflow

1. Run `cargo budget-report` against Soroban testnet to produce a Tier B JSON
   report (`/tmp/tier_b_report.json`).
2. Run `cargo budget-report --derive-limits` with the Tier B JSON as input.
3. The derivation pipeline reads each `(package, function, metric)` triple from
   the Tier B report, applies the configured margin (e.g. 3.0× for write bytes),
   and computes `tier_a_limit = ceil(tier_b_value × margin)`.
4. The result is written to `tier-a-limits.env` (`.env`-shaped key=value pairs)
   and `tier-a-limits.provenance.md` (a human-readable markdown table).

### The provenance mechanism

The `tier-a-limits.provenance.md` file is auto-generated and checked in. It
records the source Tier B JSON path, the margin values, and one row per
derived limit showing the key, Tier B value, margin, and resulting Tier A
limit. For write bytes, the margin is 3.0× — the highest of the four metrics
because write-bytes local estimates can underestimate network costs by the
largest margin.

When you add a new metric, you must:

1. Add its margin to the `Margin` struct in `derive.rs` (and to the CLI/TOML
   parsing that feeds it).
2. Add the metric to `metric_to_env_segment()` so the derivation pipeline
   emits the correct env-var key.
3. Re-run `cargo budget-report --derive-limits` to regenerate
   `tier-a-limits.env` and `tier-a-limits.provenance.md` with the new metric's
   rows.

The provenance file is the audit trail: a reviewer can trace any Tier A limit
back to its Tier B source and margin without opening code.

---

## Summary of what the write-bytes trace revealed

The write-bytes metric touches these concrete locations:

| Crate | File | What | Lines (approx.) |
|---|---|---|---|
| `budget-core` | `lib.rs` | `write_limit` field on `FunctionConfig` | ~68 |
| `budget-core` | `lib.rs` | `"Write Bytes" => func_config.write_limit` arm | ~78 |
| `budget-macros` | `lib.rs` | `budget_write_bytes_lt` proc-macro attribute | ~1120 |
| `cargo-budget-report` | `main.rs` | `write_limit` field on `FunctionConfig` | ~391 |
| `cargo-budget-report` | `main.rs` | `"Write Bytes" => func_config.write_limit` arm | ~588 |
| `cargo-budget-report` | `main.rs` | `write_bytes` field on `Resources` | ~353 |
| `cargo-budget-report` | `main.rs` | `write_bytes` field on `MeasuredResources` | ~401 |
| `cargo-budget-report` | `main.rs` | `("Write Bytes", write_bytes)` in report loop | ~1665 |
| `cargo-budget-report` | `main.rs` | `"Write Bytes"` in `emit_check_failure_entries` | ~628 |
| `cargo-budget-report` | `compare.rs` | `write_bytes` field on `Measurement` | ~31 |
| `cargo-budget-report` | `limit_checks.rs` | `check_write_bytes()` function + tests | ~23 |
| `cargo-budget-report` | `derive.rs` | `"Write Bytes" => Some(self.write)` margin | ~111 |
| `cargo-budget-report` | `derive.rs` | `"Write Bytes" => "WRITE"` env segment | ~522 |
| `cargo-budget-report` | `derive.rs` | `"Write Bytes"` in scenario loop | ~342 |
| `cargo-budget-report` | `validate.rs` | `write_bytes` in validation comparison | ~124 |
| `cargo-budget-report` | `watch.rs` | `"write_bytes"` in metrics array + `get_metric` | ~119, ~162 |
| `docs` | `measurements.md` | Write-bytes measurement section | — |
