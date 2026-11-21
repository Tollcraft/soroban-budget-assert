# budget-macros

The procedural macros that turn a declared budget limit into an assertion inside
a test body: `budget_cpu_lt`, `budget_mem_lt`, `budget_lt`, `budget_events_lt`,
`budget_ledger_entries_lt`, `budget_read_bytes_lt`, `budget_write_bytes_lt` and
`budget_scaling`. Each attribute expands the item it is written on, so the test
keeps its shape and only a generated tail checks the cost.

## Layout

| Path | Role |
| --- | --- |
| `src/lib.rs` | attribute definitions, limit parsing, and the code generators |
| `tests/ui.rs` | the trybuild driver plus the snapshot-input unit tests |
| `tests/fixture_inventory.rs` | this README's parity with the fixtures on disk |
| `tests/ui_matrix.rs` | the attribute-by-limit-form record of which pass fixture covers which cell |
| `tests/ui/*.rs` | compile-fail fixtures, one per rejected input |
| `tests/ui/*.stderr` | the pinned diagnostic for each compile-fail fixture |
| `tests/ui/pass/*.rs` | pass fixtures: they must compile *and* run |
| `tests/ui/support/mock_env.rs` | the stand-in `Env` and `soroban_sdk` the pass fixtures build against |
| `tests/ui/support/pass_env_file*.env` | the env files the `env_file` fixtures read, one per lookup case |
| `tests/ui/support/pass_read_bytes_env_file_baseline.env` | the two read-bytes limits the baseline-plus-env-file fixture compares against |

## Running the fixtures

```sh
cargo test -p budget-macros                           # trybuild + matrix + inventory + unit tests
cargo test -p budget-macros --test ui                 # trybuild only
cargo test -p budget-macros --test ui_matrix          # the coverage record only
cargo test -p budget-macros --test fixture_inventory  # this README only
```

`tests/ui.rs` runs two non-recursive globs, `t.compile_fail("tests/ui/*.rs")`
and `t.pass("tests/ui/pass/*.rs")`. A compile-fail fixture passes only when the
text in its `.stderr` matches; a pass fixture is compiled *and executed*, so the
runtime assertions in its `main` are what actually pin the diagnostic.

Adding a pass fixture is therefore a three-file change: the fixture, its row
below, and the cell in `tests/ui_matrix.rs` that claims it. That table derives
the fixtures per attribute and limit form from disk, so a new fixture no cell
names fails there before it fails anywhere else.

## Regenerating a snapshot

After a deliberate change to a macro's diagnostic, re-record the affected
snapshots and read the diff before committing:

```sh
TRYBUILD=overwrite cargo test -p budget-macros
```

Never overwrite blindly. A snapshot that changes for a reason you cannot explain
is a behaviour change in the macro, not a documentation fix, and belongs in a PR
with the tests that justify it.

A compile-fail fixture with no `.stderr` at all is silently reported as passing
by trybuild, which is why `tests/ui.rs` asserts that every snapshot exists and
`tests/fixture_inventory.rs` asserts that each snapshot carries the text quoted
below.

## Environment variables and fixture files

The compile-fail `env` fixtures need `UI_TEST_INVALID_BUDGET` and
`UI_TEST_UNSET_BUDGET`; `tests/ui.rs` sets the first to a non-`u64` value and
leaves the second unset, which is the condition each fixture names. The pass
`env` fixtures read `UI_TEST_MAX_CPU`, `UI_TEST_MAX_CPU_EXCEEDED`,
`UI_TEST_MAX_CPU_UNPARSABLE` and `UI_TEST_MAX_CPU_WHITESPACE`, also set by
`tests/ui.rs`.

`tests/ui/support/pass_env_file.env` carries:

- `TINY_LIMIT=1000000` — a limit no mock cost crosses, so the passing cases
  cannot fail for the wrong reason.
- `PCT_TEST_CPU_NETWORK=10000` and `PCT_TEST_MEM_NETWORK=20000` — the reference
  limits a percentage is taken of. `pct = 25` of the first resolves to `2500`.
- `PCT_TEST_SMALL_NETWORK=50` — the zero-resolution case: `50 * 1 / 100` is `0`
  under the integer `reference * pct / 100` arithmetic.

`tests/ui/support/pass_env_file_duplicate_key.env` holds every key twice, once
satisfiable and once not, so the two mirror-image cases can only both pass under
first-match semantics (#997). `tests/ui/support/pass_env_file_hash_suffix.env`
writes each value with and without a space before its inline `#`, which is the
separator the generated reader looks for (#998). `tests/ui/env_file_malformed.env`
carries the lines with no `=` at all that the lookup is expected to skip (#952).
`tests/ui/support/pass_read_bytes_env_file_baseline.env` carries
`READ_BYTES_LOOSE_LIMIT=500` and `READ_BYTES_TIGHT_LIMIT=100`, read alongside a
`baseline = 1_000`: the loose key passes a marginal 200 and fails a marginal 900,
while the tight key fails that same marginal 200 and passes a measured 1_050 —
which only holds if the baseline comes off before the file's limit is compared
(#1007).

## Compile-fail fixtures

All 74 compile-fail fixtures appear here, one row per fixture. The last column is a
substring taken verbatim from the fixture's `.stderr` and asserted by
`fixture_inventory.rs` to still appear there.

| Fixture | Guards against | Quoted text the fixture pins |
| --- | --- | --- |
| `tests/ui/budget_lt_baseline_property.rs` | a bare `baseline` on the combined attribute, which only has `cpu_baseline` and `mem_baseline` | unknown property: baseline |
| `tests/ui/budget_lt_cpu_baseline_no_cpu.rs` | `cpu_baseline` with no `cpu` limit, so it would silently do nothing | `cpu_baseline` requires a `cpu` limit |
| `tests/ui/budget_lt_config_source.rs` | a top-level `config =` where the source belongs nested under a metric | `config` names a limit source, not a property |
| `tests/ui/budget_lt_duplicate_config.rs` | the same `config` key twice in one limit | `config` cannot be specified more than once |
| `tests/ui/budget_lt_duplicate_env.rs` | the same `env` key twice in one limit | `env` cannot be specified more than once |
| `tests/ui/budget_lt_duplicate_env_file.rs` | the same `env_file` path twice in one limit | `env_file` cannot be specified more than once |
| `tests/ui/budget_lt_duplicate_metric.rs` | the same `cpu` metric twice in one limit, where the later value quietly won (#987) | `cpu` cannot be specified more than once |
| `tests/ui/budget_lt_env_file_source.rs` | a top-level `env_file =` + `env =` pair where the metric nesting was expected | `env_file` names a limit source, not a property |
| `tests/ui/budget_lt_env_source.rs` | a bare `env = …` where a `cpu` or `mem` property was expected | `env` names a limit source, not a property — use `cpu = env = …` or `mem = env = …` |
| `tests/ui/budget_lt_mem_baseline_no_mem.rs` | `mem_baseline` with no `mem` limit, same rationale as the CPU side | `mem_baseline` requires a `mem` limit |
| `tests/ui/budget_lt_no_metrics.rs` | the combined attribute given neither `cpu` nor `mem` | must provide at least one of `cpu` or `mem` limits |
| `tests/ui/budget_lt_pct_no_of.rs` | `pct` inside the combined attribute without the `of` reference | `pct` requires `of = <source>` — e.g. |
| `tests/ui/budget_lt_positional_literal.rs` | a positional literal, which the spec parser reads as the first key | expected identifier |
| `tests/ui/budget_lt_unknown_property.rs` | a property outside the accepted `budget_lt` key set | unknown property: unknown_prop |
| `tests/ui/env_file_missing_file.rs` | an `env_file` path that exists nowhere at expansion time | env_file "definitely/not/a/real/limits.env" was not found at macro-expansion time (looked |
| `tests/ui/env_file_no_env.rs` | `env_file` with no `env` key to read out of it | `env_file` must be paired with `env = "VAR_NAME"` |
| `tests/ui/env_file_non_string.rs` | an `env_file` value that is not a path: an integer at the binding, an empty string refused during expansion | expected `&str`, found integer |
| `tests/ui/env_file_with_config.rs` | `env_file` combined with `config`, two sources for one limit | `env_file` cannot be paired with `config` — use `env_file = "PATH", env = "VAR_NAME"` instead |
| `tests/ui/env_var_invalid.rs` | an `env`-sourced limit whose value is not a `u64` | env var `UI_TEST_INVALID_BUDGET` has value "not-a-u64", which is not a valid u64 |
| `tests/ui/env_var_unset.rs` | an `env`-sourced limit for a variable that is not set | env var `UI_TEST_UNSET_BUDGET` is not set at macro-expansion time |
| `tests/ui/env_with_config.rs` | `env` combined with `config`, two sources for one limit | `env` and `config` cannot be combined — pick one |
| `tests/ui/events_invalid_arg.rs` | the events attribute given a key that is not a limit source | expected `env`, `env_file`, `config`, or `pct`, got `wrong` |
| `tests/ui/events_no_arg.rs` | the events attribute with an empty argument list | expected an integer literal, `env = "VAR"`, `env_file = "PATH"` + `env = "VAR"` |
| `tests/ui/fail_return_in_macro.rs` | a `return` written inside macro tokens, which cannot be rewritten | `return` inside a macro invocation is not supported by the budget macros: the assertion cannot |
| `tests/ui/fail_return_in_macro_closure.rs` | the same `return`, owned by a closure inside the macro tokens | `return` inside a macro invocation is not supported by the budget macros: the assertion cannot |
| `tests/ui/impl_block_skip_all_eight.rs` | an impl block whose every method already has its own budget attribute | #[budget_*] on this impl block instrumented no methods: it has no functions, or every function |
| `tests/ui/invalid_arg.rs` | the CPU attribute given a key that is not a limit source | expected `env`, `env_file`, `config`, or `pct`, got `wrong` |
| `tests/ui/ledger_invalid_arg.rs` | the ledger attribute given a key that is not a limit source | expected `env`, `env_file`, `config`, or `pct`, got `wrong` |
| `tests/ui/ledger_no_arg.rs` | the ledger attribute with an empty argument list | expected an integer literal, `env = "VAR"`, `env_file = "PATH"` + `env = "VAR"` |
| `tests/ui/missing_env.rs` | a body with no `env` binding and no `env_ident` override | expected value, found macro `env` |
| `tests/ui/missing_env_budget_lt.rs` | the same missing binding through the combined attribute | expected value, found macro `env` |
| `tests/ui/missing_env_events.rs` | the same missing binding through the events attribute | expected value, found macro `env` |
| `tests/ui/missing_env_ledger.rs` | the same missing binding through the ledger attribute | expected value, found macro `env` |
| `tests/ui/missing_env_mem.rs` | the same missing binding through the memory attribute | expected value, found macro `env` |
| `tests/ui/missing_env_read.rs` | the same missing binding through the read-bytes attribute | expected value, found macro `env` |
| `tests/ui/missing_env_write.rs` | the same missing binding through the write-bytes attribute | expected value, found macro `env` |
| `tests/ui/no_arg.rs` | the CPU attribute with an empty argument list | expected an integer literal, `env = "VAR"`, `env_file = "PATH"` + `env = "VAR"` |
| `tests/ui/on_empty_impl.rs` | an impl block with no methods, so nothing would be instrumented | #[budget_*] on this impl block instrumented no methods: it has no functions, or every function |
| `tests/ui/on_struct.rs` | the CPU attribute on a unit struct instead of a function | expected `fn` |
| `tests/ui/on_struct_budget_lt.rs` | the combined attribute on a struct | expected `fn` |
| `tests/ui/on_struct_config_limit.rs` | a `config`-sourced limit on a struct | expected `fn` |
| `tests/ui/on_struct_env_limit.rs` | an `env`-sourced limit on a struct | expected `fn` |
| `tests/ui/on_struct_events.rs` | the events attribute on a struct | expected `fn` |
| `tests/ui/on_struct_ledger.rs` | the ledger attribute on a struct | expected `fn` |
| `tests/ui/on_struct_mem_lt.rs` | the memory attribute on a struct | expected `fn` |
| `tests/ui/on_struct_named.rs` | a struct with named fields, rejected the same way | expected `fn` |
| `tests/ui/on_struct_pct_limit.rs` | a `pct` limit source on a struct | expected `fn` |
| `tests/ui/on_struct_read_bytes.rs` | the read-bytes attribute on a struct | expected `fn` |
| `tests/ui/on_struct_scaling.rs` | `budget_scaling` on a struct | expected `fn` |
| `tests/ui/on_struct_tuple.rs` | a tuple struct, rejected by the same non-function failure | expected `fn` |
| `tests/ui/on_struct_with_baseline.rs` | a struct carrying a `baseline = …` argument | expected `fn` |
| `tests/ui/on_struct_write_bytes.rs` | the write-bytes attribute on a struct | expected `fn` |
| `tests/ui/pct_no_of.rs` | `pct` with no `of` reference limit | `pct` requires `of = <source>` — e.g. |
| `tests/ui/pct_out_of_range.rs` | a percentage below the valid range | percentage must be between 1 and 100, got 0 |
| `tests/ui/pct_out_of_range_high.rs` | a percentage above the valid range | percentage must be between 1 and 100, got 101 |
| `tests/ui/pct_with_env.rs` | an integer literal combined with `pct`, two answers for one limit | integer literal cannot be combined with env / config / env_file / pct |
| `tests/ui/read_bytes_no_arg.rs` | the read-bytes attribute with an empty argument list | expected an integer literal, `env = "VAR"`, `env_file = "PATH"` + `env = "VAR"` |
| `tests/ui/scaling_bad_sizes_descending.rs` | `sizes` that decrease, so the next ratio is undefined | `sizes` must be strictly ascending with distinct values, got 10 after 100 |
| `tests/ui/scaling_bad_sizes_duplicate.rs` | `sizes` with a repeated value, giving a trivial ratio of one | `sizes` must be strictly ascending with distinct values, got 10 after 10 |
| `tests/ui/scaling_bad_sizes_zero.rs` | a zero in `sizes`, making the expected ratio infinite | `sizes` must contain only strictly positive values, got 0 |
| `tests/ui/scaling_bad_tolerance.rs` | a negative tolerance, failing every check for the wrong reason | `tolerance` must be a finite value greater than or equal to zero, got -0.5 |
| `tests/ui/scaling_duplicate_model.rs` | `model` given twice in one scaling block, where the later value quietly won (#987) | `model` cannot be specified more than once |
| `tests/ui/scaling_missing_model.rs` | scaling configuration with no `model` | unexpected end of input, missing required field `model` |
| `tests/ui/scaling_missing_sizes.rs` | scaling configuration with no `sizes` | unexpected end of input, missing required field `sizes` |
| `tests/ui/scaling_missing_tolerance.rs` | scaling configuration with no `tolerance` | unexpected end of input, missing required field `tolerance` |
| `tests/ui/scaling_model_case.rs` | a model name differing only by case, since matching is case-sensitive | unknown growth model `Linear`, expected `linear` or `quadratic` |
| `tests/ui/scaling_single_size.rs` | `sizes` with one element, no ratio to compare | `sizes` must contain at least two elements for ratio comparison |
| `tests/ui/scaling_too_few_sizes.rs` | `sizes` below the two-element minimum for ratio comparison | `sizes` must contain at least two elements for ratio comparison |
| `tests/ui/scaling_unknown_model.rs` | a growth model that is neither `linear` nor `quadratic` | unknown growth model `exponential`, expected `linear` or `quadratic` |
| `tests/ui/scaling_unknown_property.rs` | a scaling key outside `sizes`, `model`, `tolerance` | unknown scaling property `steps`, expected `sizes`, `model`, or `tolerance` |
| `tests/ui/standalone_missing_comma.rs` | a standalone limit and its `baseline` run together, which is not valid syntax | expected `,` |
| `tests/ui/trailing_tokens_after_baseline.rs` | a key other than `baseline` / `env_ident` following a standalone `baseline = …` | expected `baseline` or `env_ident`, got `extra` |
| `tests/ui/unreachable_still_reported.rs` | genuine dead code after a `return` stays reported | unreachable statement |
| `tests/ui/wrong_baseline_key.rs` | a near-miss for the `baseline` key in a standalone attribute | expected `baseline` or `env_ident`, got `baselines` |

## Pass fixtures

All 30 pass fixtures appear here, one row per fixture. The last column is a
substring taken verbatim from the fixture's own source: the diagnostic text it
asserts at runtime, or the attribute form it exercises.

| Fixture | Guards against | Quoted text the fixture pins |
| --- | --- | --- |
| `tests/ui/pass/pass_baseline.rs` | `baseline = …`, `cpu_baseline` and `mem_baseline` subtract a floor before the comparison | CPU instruction cost 900 exceeded limit 500 |
| `tests/ui/pass/pass_budget_lt.rs` | the `budget_lt` argument forms the reference page documents, each limit nested under its metric (#808) | #[budget_lt(cpu = 1_000_000, cpu_baseline = instantiation_floor(), mem = 500, mem_baseline = 100)] |
| `tests/ui/pass/pass_budget_lt_mixed_sources.rs` | one `#[budget_lt]` taking CPU from a percentage and memory from an env-file value, each with its own baseline (issue #995) | 25% of network limit |
| `tests/ui/pass/pass_config_limit.rs` | a `config`-sourced limit, which accepts only non-negative JSON integers | #[budget_cpu_lt(config = "cpu_limit")] |
| `tests/ui/pass/pass_early_return.rs` | an early `return` still runs the assertion on every path out of the body | CPU instruction cost 1001 exceeded limit 1000 |
| `tests/ui/pass/pass_env_file.rs` | the `env_file = "PATH"` plus `env = "VAR"` limit form | #[budget_cpu_lt(env_file = ENV_FILE, env = "TINY_LIMIT")] |
| `tests/ui/pass/pass_env_file_duplicate_key.rs` | a key written twice in one env file, where the first matching line wins (#997) | CPU instruction cost 5000 exceeded limit 100 |
| `tests/ui/pass/pass_env_file_hash_suffix.rs` | an inline env-file comment with no space before the `#`, which stays part of the value (#998) | 1000000#derived-by-cargo-budget-report |
| `tests/ui/pass/pass_env_file_malformed.rs` | a malformed env-file line is skipped rather than aborting the lookup (#952) | env_file = "tests/ui/env_file_malformed.env" |
| `tests/ui/pass/pass_env_ident_standalone.rs` | `env_ident` on the six standalone attributes, before or after `baseline` (#989) | #[budget_cpu_lt(1_000, baseline = floor(), env_ident = e)] |
| `tests/ui/pass/pass_env_var_limit.rs` | the `env = "VAR"` form, resolved at expansion and re-read at runtime | #[budget_cpu_lt(env = "UI_TEST_MAX_CPU")] |
| `tests/ui/pass/pass_events.rs` | the events attribute across every supported body shape | #[budget_events_lt(10)] |
| `tests/ui/pass/pass_events_env_file.rs` | the events attribute reading its limit from an env file, the `EnvFile` arm outside the CPU path | #[budget_events_lt(env_file = ENV_FILE, env = "TINY_LIMIT")] |
| `tests/ui/pass/pass_events_ledger_impl_block.rs` | the events and ledger-entry attributes at `impl`-block level, method overrides included (#988) | Ledger entry count (read: 7, write: 4, total: 11) exceeded limit 6 |
| `tests/ui/pass/pass_impl_block.rs` | an attribute on an `impl` block instruments every method in it | #[budget_cpu_lt(18446744073709551615)] |
| `tests/ui/pass/pass_impl_env_ident.rs` | `env_ident` reaching every method of an annotated `impl` block (issue #996) | #[budget_lt(cpu = 1_000, env_ident = e)] |
| `tests/ui/pass/pass_ledger_baseline.rs` | `baseline = …` on the ledger attribute, where the sum is read first and the marginal total compared | #[budget_ledger_entries_lt(1, baseline = setup_floor())] |
| `tests/ui/pass/pass_ledger_breakdown.rs` | the exact ledger failure text, including the read/write breakdown (issue #1000) | Ledger entry count (read: 7, write: 3, total: 10) exceeded limit 9 |
| `tests/ui/pass/pass_ledger_entries.rs` | the ledger-entry attribute across every supported body shape | #[budget_ledger_entries_lt(50)] |
| `tests/ui/pass/pass_mem.rs` | the memory attribute shares the CPU expansion, so it gets the same body shapes | #[budget_mem_lt(4_096)] |
| `tests/ui/pass/pass_nested_pct.rs` | a nested `cpu = pct = N, of = …` beside a sibling literal `mem` limit | cpu = pct = 25, |
| `tests/ui/pass/pass_percentage_limit.rs` | `pct = N, of = …` resolves the limit from a reference read at runtime | pct = 25, of = env_file = PCT_ENV_FILE |
| `tests/ui/pass/pass_read_bytes.rs` | the read-bytes attribute body shapes, reported through `memory_bytes_cost()` | #[budget_read_bytes_lt(4_096)] |
| `tests/ui/pass/pass_read_bytes_baseline.rs` | `baseline = …` on the read-bytes attribute, so the marginal cost is asserted | Read bytes cost (memory proxy) 900 exceeded limit 500 |
| `tests/ui/pass/pass_read_bytes_env_file_baseline.rs` | an env-file limit *and* a `baseline` on one read-bytes attribute, with the subtraction applied before the file's limit (issue #1007) | #[budget_read_bytes_lt(env_file = ENV_FILE, env = "READ_BYTES_LOOSE_LIMIT", baseline = floor())] |
| `tests/ui/pass/pass_result.rs` | a `Result` body keeps its tail value and its `?` operators | let value = doubled(21)?; |
| `tests/ui/pass/pass_scaling.rs` | `#[budget_scaling]` on a function whose cost grows with its size parameter | #[budget_scaling(sizes = [100, 1000, 10000], model = linear, tolerance = 1.0)] |
| `tests/ui/pass/pass_stacked_metrics.rs` | two budget attributes stacked on one function, each still catching its own breach (#994) | Memory bytes cost 90000 exceeded limit 8192 |
| `tests/ui/pass/pass_unit.rs` | a plain `()` body: the assertion runs after the last statement | CPU instruction cost 1000 exceeded limit 1000 |
| `tests/ui/pass/pass_write_bytes.rs` | the write-bytes attribute body shapes, reported through `memory_bytes_cost()` | #[budget_write_bytes_lt(4_096)] |

## Keeping this inventory honest

`tests/fixture_inventory.rs` re-reads this README and the two fixture
directories and fails when:

- a fixture on disk has no row, or a row names a fixture that is gone,
- a fixture is listed twice,
- a quoted substring no longer appears in the `.stderr` (compile-fail) or in the
  fixture source (pass),
- the counts stated above stop matching the directories,
- the regeneration command or the inventory command stops being documented.

Adding, renaming or rewording a fixture is therefore a two-file change: the
fixture and its row. `cargo test -p budget-macros --test fixture_inventory` is
the check that catches the missing half.
