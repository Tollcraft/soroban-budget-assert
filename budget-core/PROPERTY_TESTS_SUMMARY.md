# Property Tests Summary

## Overview

The property tests for the hand-rolled attribute `Parse` impls live in
`budget-macros/src/parser_props.rs`. The module covers the four spec types
`StandaloneSpec`, `BudgetLimit`, `BudgetSpec`, and `ScalingConfig`, and is
wired in as `#[cfg(test)] mod parser_props;` in `budget-macros/src/lib.rs`,
so it compiles only during `cargo test`.

**Core property:** no input causes a panic. The parsers may reject any input
they like — but as a `syn::Error`, never by unwinding. In a procedural
macro an unwind surfaces as an opaque "proc macro panicked" error with
nothing pointing at the offending attribute, so an unwind is always a bug
even when the input is nonsense. The generators therefore build *plausible*
attribute syntax — real keys, real shapes, deliberately broken values,
reordered / duplicated / truncated forms — rather than random bytes.

## Configuration

- **proptest version**: `1.5` (dev-dependency of `budget-macros`, declared in `budget-macros/Cargo.toml:30`)
- **Test cases per property**: `600` (`ProptestConfig::with_cases(600)`, `budget-macros/src/parser_props.rs:200`)
- **Location**: `budget-macros/src/parser_props.rs`, inside the `proptest!` block that starts at line 199

## Coverage

Five property tests, all in the `proptest!` block:

| Property test | Spec type | What it asserts |
| :--- | :--- | :--- |
| `standalone_spec_never_panics` | `StandaloneSpec` | Any generated standalone spec either parses or is rejected as a `syn::Error` — never a panic |
| `budget_limit_never_panics` | `BudgetLimit` | Same property for generated limit sources (bare integers, `key = value` forms, broken values) |
| `budget_spec_never_panics` | `BudgetSpec` | Same property for generated budget spec sources (`cpu` / `mem` clauses, baselines, unknown and duplicate keys) |
| `scaling_config_parses_only_with_a_usable_shape` | `ScalingConfig` | Parsing never panics **and** any parsed config satisfies the parser's invariants: `sizes` has at least two elements, `tolerance` is finite |
| `int_budget_limit_round_trips` | `BudgetLimit` | A bare `u64` that parses is re-emitted through `generate_limit_expr` and re-parses to an equal integer value |

**Total: 5 property tests.**

## Companion regression tests

The same file also keeps fixed inputs as ordinary deterministic tests in
`mod regressions` (`budget-macros/src/parser_props.rs:271`). These are *not*
property tests; they pin cases a property run found (or nearly found) so they
stay covered independently of the random run:

| Regression test | Pins |
| :--- | :--- |
| `pct_of_non_env_file_parses_without_parser_panic` | `pct = 50, of = <non-env_file>` parses cleanly at the parser level |
| `overflowing_int_is_rejected_cleanly` | Numeric overflow in a limit literal is a `syn::Error`, not a panic |
| `scaling_sizes_u32_overflow_is_rejected_cleanly` | `sizes` entries that overflow `u32` are a `syn::Error`, not a panic |

Anything the properties surface that needs a parser or diagnostics fix
belongs to #394 (macro diagnostics + ui tests), not to this suite.

## Running the tests

From the workspace root:

```bash
cargo test -p budget-macros
```

To run only the property tests and their companion regressions:

```bash
cargo test -p budget-macros parser_props
```

To run a single property:

```bash
cargo test -p budget-macros int_budget_limit_round_trips
```
