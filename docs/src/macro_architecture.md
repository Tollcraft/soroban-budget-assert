# Macro Expansion Architecture

This document describes how the `budget-macros` procedural macro crate transforms annotated test functions at compile time. It is intended for contributors extending or modifying the macro system.

## High-Level Architecture

The macro crate is a single proc-macro library at `budget-macros/src/lib.rs`. It exports four attribute macros:

| Macro | Metric Asserted | Accessor |
|---|---|---|
| `#[budget_cpu_lt(N)]` | CPU instructions | `budget.cpu_instruction_cost()` |
| `#[budget_mem_lt(N)]` | Memory bytes | `budget.memory_bytes_cost()` |
| `#[budget_lt(cpu = N, mem = M)]` | Both (combined) | Both accessors |
| `#[budget_write_bytes_lt(N)]` | Write bytes (via memory proxy) | `budget.memory_bytes_cost()` |

All four share a common expansion pipeline. The first three flow through `generate_budget_assert()`; `budget_write_bytes_lt` duplicates the instrumentation pattern inline to keep its distinct metric label and proxy caveat self-contained.

```
                           ┌─────────────────────────────┐
                           │     rustc compiles test      │
                           │   #[budget_cpu_lt(950_000)]  │
                           │   fn my_test() { ... }      │
                           └──────────────┬──────────────┘
                                          │
                                          ▼
                     ┌─────────────────────────────────────┐
                     │  1. Parse attribute tokens          │
                     │     BudgetLimit::Int(950_000)       │
                     └──────────────────┬──────────────────┘
                                          │
                                          ▼
                     ┌─────────────────────────────────────┐
                     │  2. Parse test function             │
                     │     syn::ItemFn                     │
                     └──────────────────┬──────────────────┘
                                          │
                                          ▼
                     ┌─────────────────────────────────────┐
                     │  3. Generate assertion tokens       │
                     │     quote! { ... assert!(...) }     │
                     └──────────────────┬──────────────────┘
                                          │
                                          ▼
                     ┌─────────────────────────────────────┐
                     │  4. Instrument exit paths           │
                     │     instrument_exit_paths()         │
                     │     ↳ ReturnRewriter for `return`s  │
                     └──────────────────┬──────────────────┘
                                          │
                                          ▼
                     ┌─────────────────────────────────────┐
                     │  5. Emit rewritten fn to rustc      │
                     └─────────────────────────────────────┘
```

### Why Procedural Macros?

The macros must inspect the test function's AST at compile time to:

- Inject a `budget_env_resolve` closure before the original statements.
- Append a budget-checking assertion on every exit path.
- Rewrite `return` expressions so the assertion fires before the value leaves.
- Detect `return` tokens hidden inside macro invocations and emit a compile error.

None of these transformations are possible with `macro_rules!` pattern matching, because the rewrite depends on the AST structure (trailing expressions, nested `return`s, closure boundaries) rather than token-sequence patterns. A proc-macro receives the parsed `TokenStream` and has access to `syn` for full AST manipulation.

### Relationship to the Workspace

```
soroban-budget-assert/
├── budget-macros/           # ← This crate — proc-macro, no runtime deps on SDK
│   ├── src/lib.rs
│   └── tests/
│       ├── ui.rs            # trybuild compile-fail + pass tests
│       ├── ui/*.rs          # per-case source files
│       └── ui/support/      # mock_env.rs for UI tests
├── amm-pool-contract/       # Integration tests that exercise macros against real Env
├── cargo-budget-report/     # CLI — no dependency on budget-macros
└── docs/src/                # GitBook documentation
```

`budget-macros` is an independent crate with no runtime dependency on the Soroban SDK. Its only production dependencies are `syn`, `quote`, `proc-macro2`, and `serde_json` (for compile-time `config = "key"` resolution). The generated code references `env.cost_estimate().budget()` by identifier, so the test function must have a binding named `env` of a type that exposes that path — typically `soroban_sdk::Env`.

## Expansion Flow

```mermaid
graph TD
    A["#[budget_cpu_lt(N)] or<br/>#[budget_cpu_lt(env = \"VAR\")]"]
    A --> B["Proc-macro invoked at compile time<br/>by rustc"]

    B --> C{Parse attribute tokens}

    C -->|Integer literal| D["BudgetLimit::Int(N)<br/>limit_expr = quote! { N }"]
    C -->|env = \"VAR\" syntax| E["BudgetLimit::EnvVar(VAR)<br/>limit_expr = quote! {<br/>  match budget_env_resolve(VAR) { … }<br/>}"]

    D --> F[Parse test fn body via syn::ItemFn]
    E --> F

    F --> G["Inject preamble into fn body:<br/>let budget_env_resolve = |var| std::env::var(var).ok()"]

    G --> H["Append cost-check epilogue:<br/>let budget = env.cost_estimate().budget();<br/>let cpu_cost = budget.cpu_instruction_cost();<br/>let limit_u64: u64 = limit_expr;<br/>assert!(cpu_cost < limit_u64, …)"]

    H --> I["Emit rewritten fn tokens<br/>back to rustc"]

    I --> J{Runtime: test executes}

    J -->|cpu_cost < limit_u64| K["Test passes ✅"]
    J -->|cpu_cost >= limit_u64| L["panic! with message:<br/>'CPU instruction cost N exceeded limit M<br/>- local estimate, real network cost<br/>may differ significantly in either direction'"]

    L --> M{#[should_panic] present?}
    M -->|Yes — deliberate regression fixture| N["Test passes ✅<br/>(expected failure documented)"]
    M -->|No — real regression| O["Test fails ❌<br/>CI exits non-zero"]

    style A fill:#1e3a5f,color:#fff,stroke:#4a90d9
    style K fill:#1a4731,color:#fff,stroke:#2d7a4f
    style N fill:#1a4731,color:#fff,stroke:#2d7a4f
    style O fill:#5c1a1a,color:#fff,stroke:#c0392b
    style L fill:#5c1a1a,color:#fff,stroke:#c0392b
```

### 1. Attribute Macro Entry Point

When rustc encounters:

```rust
#[budget_cpu_lt(950_000)]
fn my_test() { /* ... */ }
```

it calls `budget_cpu_lt` (line 504) with two token streams:
- `attr` — the tokens inside the attribute parentheses: `950_000`
- `item` — the tokens of the annotated item: `fn my_test() { ... }`

### 2. Token Parsing

The attribute argument is parsed into a `BudgetLimit` enum via its `syn::parse::Parse` implementation (line 33):

```
#[budget_cpu_lt(950_000)]       → BudgetLimit::Int(950_000)
#[budget_cpu_lt(env = "VAR")]   → BudgetLimit::EnvVar("VAR".into())
#[budget_cpu_lt(config = "k")]  → BudgetLimit::Config("k".into())
```

For `budget_lt` (combined CPU + mem), the attribute is parsed as a `BudgetSpec` (line 54):

```
#[budget_lt(cpu = 1_000_000, mem = 500_000)]
```

`BudgetSpec` holds optional `cpu` and `mem` limits plus an optional `env_ident` override. At least one of `cpu` or `mem` is required; the parser returns a compile error otherwise.

### 3. Function Parsing

The `item` token stream is parsed as `syn::ItemFn` (`lib.rs:373`). If parsing fails (e.g., the macro is placed on a struct), the error is returned directly to rustc.

### 4. Limit Expression Generation

`generate_limit_expr()` (line 122) converts each `BudgetLimit` variant into a `TokenStream`:

- **`Int(n)`** — emits the literal `n` as a token stream. Zero runtime cost.
- **`EnvVar(var)`** — emits a call to the injected `budget_env_resolve` helper, which reads `std::env::var(var)` at runtime. Falls back to `u64::MAX` when unset.
- **`Config(key)`** — attempts compile-time JSON resolution of `budget.json` via `serde_json`. If the file exists and is well-formed, the value is emitted as a literal (zero runtime cost). Otherwise, a runtime JSON parser (using `std` only, no `serde`) is emitted as a fallback.

### 5. Assertion Construction

For each limit present, an `assert!` block is built (lines 384–416):

```rust
// CPU variant (generated tokens, not source)
let cpu_cost = budget.cpu_instruction_cost();
let limit_u64: u64 = <limit_expr>;
assert!(
    cpu_cost < limit_u64,
    "CPU instruction cost {} exceeded limit {} - local estimate, ...",
    cpu_cost,
    limit_u64
);
```

The memory variant uses `budget.memory_bytes_cost()` and a corresponding message.

### 6. Preamble Injection

A preamble closure is prepended to the function body (line 419):

```rust
#[allow(unused_variables)]
let budget_env_resolve = |var: &str| -> Option<String> {
    std::env::var(var).ok()
};
```

This closure is injected in the function's scope so that limit forms using `env = "VAR"` can read environment variables. Tests may shadow `budget_env_resolve` to supply synthetic values — the shadowing binding wins because the assertion resolves names in body scope at the exit point.

### 7. Exit Path Instrumentation

`instrument_exit_paths()` (line 298) is the core of the rewrites. It handles three body shapes:

**Trailing expression (e.g., `Ok(())`):**

```rust
// Before:
fn test() -> Result<(), Error> {
    // ... statements ...
    Ok(())
}

// After:
fn test() -> Result<(), Error> {
    // preamble
    // ... statements ...
    let __budget_value = Ok(());
    // assertion block
    __budget_value
}
```

The trailing expression is bound to `__budget_value`, the assertion runs, then the value is yielded — preserving the `Result` type.

**No trailing expression (unit body):**

```rust
// Before:
fn test() {
    // ... statements ...
}

// After:
fn test() {
    // preamble
    // ... statements ...
    // assertion block
}
```

**Early `return`:**

A `ReturnRewriter` (line 230, implementing `syn::visit_mut::VisitMut`) walks the AST and rewrites every `return` so the assertion fires before the value leaves:

```rust
// Before:
return value;

// After:
return {
    let __budget_returned = value;
    // assertion block
    __budget_returned
};
```

This preserves the `!` type of `return` so type-checking still works for diverging paths. Returns inside closures or `async` blocks are left untouched — they exit that inner body, not the test function. Returns inside nested items (helper `fn`s declared in the body) are similarly skipped.

**Returns hidden in macro tokens:**

If a `return` token is found inside a macro invocation (e.g., inside `assert!(if cond { return Err(..) } else { .. })`), the rewrite cannot reach it. The `find_return_token()` helper (line 276) recursively searches the macro's token tree, and if found, the macro emits a compile error pointing at the problematic `return` rather than silently skipping the check.

### 8. Reassembly and Emission

The rewritten function body is spliced back into the `ItemFn`. `#[allow(unreachable_code)]` is appended to the function attributes (line 367) to suppress warnings when a trailing `return` or `panic!` makes the assertion technically unreachable — the paths that do reach an exit already ran the check.

The final token stream is the complete `ItemFn`:

```rust
#[allow(unreachable_code)]
fn my_test() {
    #[allow(unused_variables)]
    let budget_env_resolve = |var: &str| -> Option<String> { /* ... */ };
    // original statements ...
    // assertion block using env.cost_estimate().budget()
}
```

### 9. Runtime Execution

When the test runs:

1. The preamble runs, defining `budget_env_resolve`.
2. The original test statements execute (contract registration, client calls).
3. After the last statement (or at each `return`), the assertion block runs:
   - Reads `env.cost_estimate().budget()`.
   - Calls `.cpu_instruction_cost()` and/or `.memory_bytes_cost()`.
   - Compares each metric against the limit.
   - Panics with a descriptive message if any metric meets or exceeds its limit.
4. If the assertion passes, the test continues normally — the return value is yielded, or the function ends.

### Compile-Time vs Runtime Responsibilities

| Phase | Work | Why |
|---|---|---|
| Compile | Parse attribute into `BudgetLimit` | Rustc feeds raw tokens; parsing must happen here |
| Compile | Parse test function as `ItemFn` | AST rewrite requires a structured parse |
| Compile | Resolve `config = "k"` from `budget.json` | Optimisation: inject literal when file exists (zero runtime cost); fallback emitted otherwise |
| Compile | Rewrite `return` expressions | Syntax transformation — impossible at runtime |
| Compile | Detect returns in macro tokens | Compile-time safety — cannot be deferred |
| Runtime | Read `env` variable's cost estimate | Real `Env` only exists at runtime |
| Runtime | Read environment variables (`env = "VAR"`) | `std::env::var` requires runtime access |
| Runtime | `assert!` evaluation | Actual cost is only known after execution |

## Generated Code Structure

Given this source:

```rust
#[budget_cpu_lt(950_000)]
fn test_swap() {
    let env = Env::default();
    let client = setup_pool(&env);
    client.swap(&user, &true, &100, &90);
}
```

The macro produces (conceptually):

```rust
#[allow(unreachable_code)]
fn test_swap() {
    #[allow(unused_variables)]
    let budget_env_resolve = |var: &str| -> Option<String> {
        std::env::var(var).ok()
    };
    let env = Env::default();
    let client = setup_pool(&env);
    client.swap(&user, &true, &100, &90);
    {
        let budget = env.cost_estimate().budget();
        let cpu_cost = budget.cpu_instruction_cost();
        let limit_u64: u64 = 950_000u64;
        assert!(
            cpu_cost < limit_u64,
            "CPU instruction cost {} exceeded limit {} - local estimate, \
             real network cost may differ significantly in either direction",
            cpu_cost,
            limit_u64
        );
    }
}
```

With an early return, the original:

```rust
#[budget_cpu_lt(950_000)]
fn test_conditional(early: bool) -> Result<(), Error> {
    let env = Env::default();
    if early {
        return Ok(());
    }
    // ... expensive path ...
    Ok(())
}
```

Becomes:

```rust
#[allow(unreachable_code)]
fn test_conditional(early: bool) -> Result<(), Error> {
    #[allow(unused_variables)]
    let budget_env_resolve = |var: &str| -> Option<String> { /* ... */ };
    let env = Env::default();
    if early {
        return {
            let __budget_returned = Ok(());
            {
                let budget = env.cost_estimate().budget();
                /* assert! ... */
                __budget_returned
            }
        };
    }
    // ... expensive path ...
    let __budget_value = Ok(());
    {
        let budget = env.cost_estimate().budget();
        /* assert! ... */
    }
    __budget_value
}
```

## Source Code Organization

The entire macro implementation lives in a single file, `budget-macros/src/lib.rs`. There are no submodules. The logical sections are:

### Data Types

| Type | Line | Role |
|---|---|---|
| `BudgetLimit` | 13 | Enum: `Int(u64)`, `EnvVar(String)`, `Config(String)` — how the limit is specified |
| `BudgetSpec` | 20 | Struct: optional `cpu`, `mem`, `env_ident` — the combined-limit attribute (`budget_lt`) |
| `ConfigResolution` | 93 | Enum: outcome of compile-time `budget.json` lookup |

### Parsing (`impl Parse`)

| Implementation | Line | Input | Output |
|---|---|---|---|
| `BudgetLimit::parse` | 33 | Attribute tokens | `BudgetLimit` variant |
| `BudgetSpec::parse` | 54 | Attribute tokens (`cpu = N, mem = M`) | `BudgetSpec` |
| — | 373 | `ItemFn::parse` on the annotated item | `syn::ItemFn` |

### Limit Expression Generation

| Function | Line | Input | Output |
|---|---|---|---|
| `generate_limit_expr` | 122 | `&BudgetLimit`, metric label | `TokenStream2` of the limit expression |
| `resolve_config_value` | 106 | `key: &str` | `ConfigResolution` (compile-time JSON read) |

### Exit Path Instrumentation

| Item | Line | Role |
|---|---|---|
| `ReturnRewriter` | 230 | `VisitMut` that rewrites `return` statements to include the assertion |
| `find_return_token` | 276 | Recursively searches a macro's token tree for a `return` identifier |
| `instrument_exit_paths` | 298 | Orchestrates the body rewrite: preamble, return rewriting, trailing-expression binding, assertion injection |

### Macro Entry Points

| Function | Line | Attribute |
|---|---|---|
| `budget_cpu_lt` | 504 | `#[budget_cpu_lt(...)]` |
| `budget_mem_lt` | 700 | `#[budget_mem_lt(...)]` |
| `budget_write_bytes_lt` | 583 | `#[budget_write_bytes_lt(...)]` |
| `budget_lt` | 725 | `#[budget_lt(cpu = N, mem = M)]` |

`budget_cpu_lt` and `budget_mem_lt` both delegate to `generate_budget_assert()` (line 372) with the appropriate `BudgetSpec` fields set. `budget_write_bytes_lt` duplicates the instrumentation inline because its metric label and proxy-caveat message differ from the standard pattern. `budget_lt` also delegates to `generate_budget_assert()` but passes a fully populated `BudgetSpec` from its composite parser.

### `generate_budget_assert()` (line 372)

This is the shared expansion workhorse. It:

1. Parses the `item` as `ItemFn`.
2. Determines the `env` identifier (defaults to `"env"`, overridable via `env_ident`).
3. For each present limit (`cpu` or `mem`), calls `generate_limit_expr()` and builds the corresponding `assert!`.
4. Constructs the preamble and assertion token streams.
5. Calls `instrument_exit_paths()` to perform the body rewrite.
6. Returns the complete `ItemFn` as a `TokenStream`.

## Design Decisions

### Single-File Organization

The entire macro implementation is intentionally kept in one file. The crate is small (four attribute macros sharing most of their logic), and splitting into modules would add file-navigation overhead without a commensurate readability gain. If the crate grows significantly (e.g., new macros with distinct infrastructure), the logical sections above (`parsing`, `codegen`, `instrumentation`) are natural module boundaries.

### Parsing Strategy

Attribute arguments use `syn`'s `Parse` trait rather than manual token-tree walking. This gives:

- Composable parsers (`BudgetLimit` feeds into `BudgetSpec`).
- Automatic error messages with span information (e.g., `expected 'env' or 'config', got 'wrong'`).
- Familiar patterns for contributors already using `syn`.

The `BudgetLimit` parser distinguishes integer literals from keyword arguments by peeking for an `Ident` token — if the first token is an identifier, it must be `env` or `config` followed by `=` and a string literal; otherwise, it must be an integer literal.

### Code Generation Strategy

The macro uses `quote!` for code generation rather than building token streams manually. This keeps the generated code readable in the macro source and maintains hygiene automatically — `quote!` tracks span information so that identifiers like `budget_env_resolve` and `__budget_returned` do not leak into the user's scope beyond the function body.

### Exit-Path Instrumentation via `VisitMut`

The `ReturnRewriter` uses `syn`'s visitor pattern (`VisitMut`) to walk the AST and rewrite `return` expressions in-place. This is chosen over a recursive function because:

- `VisitMut` automatically recurses into nested blocks, `if`/`match` arms, and loops.
- Closure and `async` body boundaries are handled by checking `Expr::Closure` / `Expr::Async` in `visit_expr_mut` and returning early.
- Nested items (helper `fn`s) are handled by `visit_item_mut` which does nothing — the default visitor would recurse into them and rewrite their returns too.

The visitor approach also makes the macro-return detection straightforward: a separate `visit_macro_mut` callback collects spans of `return` tokens found inside macro invocations.

### Macro Hygiene

Several decisions preserve Rust's hygiene guarantees:

- The preamble closure `budget_env_resolve` is injected in body scope so it can be shadowed by user code. Tests wanting synthetic limits simply define their own `let budget_env_resolve = ...` before use.
- The temporary variables `__budget_returned` and `__budget_value` use double-underscore prefixed names to minimise collision risk with user variables.
- The assertion block is a separate `{ }` scope so that local bindings (`budget`, `cpu_cost`, `limit_u64`) do not leak into the user's scope.
- `#[allow(unreachable_code)]` is added to the function rather than wrapping individual blocks, because the unreachable warning would otherwise appear at the call site.

### `budget_write_bytes_lt` as a Standalone Implementation

`budget_write_bytes_lt` duplicates the instrumentation pattern rather than reusing `generate_budget_assert()`. This is intentional: its metric label and the proxy-caveat in the panic message differ from the CPU/mem pattern, and the shared helper would need additional parameters to accommodate them. The duplication is localised to a single function and follows the same structural conventions, making it straightforward to refactor into the shared path if more proxy metrics are added later.

### `config = "key"` Duplicate Resolution

Both `generate_budget_assert()` (via `generate_limit_expr()`) and `budget_write_bytes_lt` compile their own JSON-parsing fallback for `Config` limits. The compile-time path (`resolve_config_value` + `serde_json`) is shared, but the runtime fallback code is emitted inline in each limit expression because `quote!` does not support emitting a shared helper function across separate expansion sites without additional abstractions. Keeping the fallback self-contained in each expansion avoids a cross-contamination risk where one macro's expansion depends on another's.

### `spec.env_ident` Flexibility

`BudgetSpec` supports an `env_ident` field that overrides the variable name used for `env.cost_estimate().budget()`. This is currently unused by the public macros (they all pass `None`, defaulting to `"env"`), but the field exists in the parser and the data structure so that a future macro or custom attribute can target a differently named variable without forking the expansion logic.

## Extension Guide

### Adding a New Macro

1. **Define the attribute entry point** — add a `#[proc_macro_attribute]` public function following the signature `fn my_macro(attr: TokenStream, item: TokenStream) -> TokenStream`.

2. **Parse the attribute** — either reuse `BudgetLimit::parse` if the argument is a single limit, or define a new `Parse` impl. If the new macro takes the same limit forms (integer, `env = "VAR"`, `config = "key"`), `BudgetLimit` can be used directly.

3. **Parse the item** — always parse as `ItemFn`. If the macro is placed on a non-function, return the parse error immediately.

4. **Build the assertion** — construct the `assert!` token stream using `quote!`. Follow the pattern from lines 389–398: bind the cost value, bind the limit, then `assert!` with a descriptive message.

5. **Instrument exit paths** — call `instrument_exit_paths()` with the function, preamble (if any), and assertion. If the function has no preamble, pass an empty `quote! {}`.

6. **Return the token stream** — emit the rewritten function: `TokenStream::from(quote! { #input_fn })`.

### Reusing Existing Infrastructure

| Task | Reuse |
|---|---|
| Parse a single integer-or-env-or-config limit | `BudgetLimit::parse` |
| Parse a composite `cpu = N, mem = M` attribute | `BudgetSpec::parse` |
| Generate a limit expression from a `BudgetLimit` | `generate_limit_expr()` |
| Instrument exit paths (return rewriting, trailing expression binding) | `instrument_exit_paths()` |
| Search for `return` inside macro tokens | `find_return_token()` |

### Avoiding Code Duplication

If the new macro's assertion logic is structurally identical to the CPU/mem pattern (single metric, straightforward `assert!`), add it to `generate_budget_assert()` by extending the `BudgetSpec` struct with an additional optional field rather than writing a separate expansion function.

If the new macro needs a different metric label, failure message, or preamble, consider parameterising `generate_budget_assert()` with a closure or config struct that describes the metric-specific parts, rather than duplicating the instrumentation logic.

### Common Pitfalls

- **Forgetting exit-path coverage** — Always use `instrument_exit_paths()` rather than appending a statement to the end of the function body. An early `return` skips a trailing assertion, making the check trivially passable.
- **Shadowing `budget_env_resolve`** — The injected closure is in body scope. If a test defines its own `budget_env_resolve`, the shadowing binding wins. This is a deliberate feature (tests can supply synthetic environment values) but can be surprising — document it.
- **`return` inside macros** — A `return` token that appears inside a macro invocation's output (e.g., `assert!(cond || return)`) cannot be rewritten. The compile-time `find_return_token` check catches these and emits a clear error. Do not add workarounds that skip the check — a silent miss would undermine the assertion's guarantees.
- **Hygiene of injected identifiers** — Use `Span::call_site()` for injected identifiers (like `budget_env_resolve` or `__budget_returned`) so they resolve in the calling crate's context. User code that happens to use these names can interact unexpectedly; the double-underscore prefix on temporaries reduces this risk.
- **`?` operator** — The `?` operator propagates errors before the assertion runs. The test still fails on the returned error (so a regression cannot pass unnoticed), but the cost is not measured on that path. Document this caveat.

### Testing Patterns

Each new macro should have:

1. **A passing integration test** in `amm-pool-contract/tests/` that exercises the macro against the real `soroban_sdk::Env` with a generous limit.
2. **A `#[should_panic]` regression test** that proves the assertion fires when the limit is breached.
3. **UI pass tests** in `budget-macros/tests/ui/pass/` using the mock `Env` from `support/mock_env.rs` — one per supported body shape (unit, `Result`-returning, early return, env-var limit).
4. **UI compile-fail tests** in `budget-macros/tests/ui/` for invalid attribute arguments (`no_arg.rs`, `invalid_arg.rs`), wrong item types (`on_struct.rs`), and detectably unsafe patterns (`fail_return_in_macro.rs`).

Run `cargo test -p budget-macros` for the UI-only tests, or `cargo test --workspace` (after building the WASM) for the full suite. Regenerate `.stderr` snapshots with `TRYBUILD=overwrite cargo test -p budget-macros`.

## `std`/`no_std` Boundary and Test-Only Status

### Investigation Result

`budget-macros` is used **exclusively in test code**. A full search of the workspace confirms:

| Consumer | File | Context |
|---|---|---|
| `amm-pool-contract` | `Cargo.toml` | `dev-dependency` only (line 15) |
| `amm-pool-contract` | `tests/budget_test.rs` | `#[cfg(test)]` test functions |
| `amm-pool-contract` | `tests/cross_contract_test.rs` | `#[cfg(test)]` test functions |
| `budget-macros` | `tests/ui/*.rs` | UI compile-test and pass-test files |

No production code, contract source, or non-test module references `budget_macros`. The crate is a **test-only utility**.

### Where `std` Is Required

The macros are a proc-macro library — they run at compile time and have no runtime. However, the **generated code** they emit references `std`:

| Generated item | `std` item | When emitted |
|---|---|---|
| `budget_env_resolve` closure | `std::env::var` | Always (preamble) |
| `EnvFile` limit expression | `std::fs::read_to_string` | When `env_file = "PATH"` is used |
| `Config` limit expression | `std::fs::read_to_string`, `std::path::Path` | When `config = "key"` is used |
| `Percentage` limit expression | `std::fs::read_to_string` | When `pct = N, of = env_file = "..."` is used |

For the common case of integer-literal limits (`#[budget_cpu_lt(950_000)]`), the only `std` reference is the `budget_env_resolve` closure in the preamble — which is defined but never called.

### Where `no_std` Applies

Soroban contracts compile to WASM and use `#![no_std]`. The macros' generated code is **never compiled into contract WASM** because the macros are dev-dependencies only. The `no_std` boundary of the contract is unaffected by `budget-macros`.

If the macros were ever used in non-test contract code, the `std::fs` and `std::env` references in the generated code would fail to compile under `no_std`. The integer-literal limit form would still work (no runtime `std` calls), but `env_file`, `config`, and `env` forms would need alternative resolution paths.

### Dependency Audit

The crate's dependencies are standard proc-macro tooling with no unnecessary `std` requirements:

| Dependency | Purpose | `std`-dependent? |
|---|---|---|
| `syn` (full, extra-traits, visit-mut) | AST parsing and manipulation | No (compile-time only) |
| `quote` | Token stream generation | No (compile-time only) |
| `proc-macro2` | Proc-macro token types | No (compile-time only) |

The crate has no runtime dependencies. `serde_json` was previously listed in the architecture doc as a dependency for `config = "key"` resolution but is not present in `Cargo.toml` — the runtime JSON parsing fallback in `generate_limit_expr` uses hand-rolled parsing (no `serde`), consistent with keeping dependencies minimal.

### How Macro Expansion Interacts with Soroban `no_std` Contracts

The macros and the contract occupy separate compilation domains:

```
┌──────────────────────────────┐     ┌──────────────────────────────┐
│  Contract (no_std, WASM)     │     │  Test binary (std, native)   │
│                              │     │                              │
│  amm-pool-contract/src/      │     │  amm-pool-contract/tests/    │
│  - No budget-macros dep      │     │  - budget_macros dev-dep     │
│  - No std references         │     │  - #[budget_cpu_lt(...)]     │
│  - Compiled to wasm32        │     │  - Generated code uses std   │
└──────────────────────────────┘     └──────────────────────────────┘
```

The contract's `no_std` boundary is maintained by the workspace dependency structure: `budget-macros` is a `[dev-dependencies]` entry, which Cargo only resolves for test and bench targets. The WASM build (`cargo build --target wasm32v1-none --release -p amm-pool-contract`) does not link `budget-macros` at all.

### Recommendations

1. **Do not add `no_std` support.** The macros are test-only and the `std` references in generated code are correct for their usage context. Adding `no_std` support would require conditional compilation gates and alternative `env_file`/`config` resolution paths with no practical benefit.

2. **Keep the test-only boundary explicit.** The Cargo.toml comment and lib.rs module docs now document this. Future contributors should not add `budget-macros` as a non-dev dependency without understanding the `std` implications.

3. **If `no_std` use cases arise later**, the integer-literal and `env` forms could be made `no_std`-compatible with minimal changes (the `env` form only needs `core::env::var` which is not available in `no_std`, so it would need a compile-time-only fallback). The `env_file` and `config` forms are inherently `std`-dependent (file I/O) and would need a `#[cfg(not(no_std))]` gate.

## Interaction with Soroban Budget Measurement

The macros do not call any Soroban SDK function directly — they have no dependency on it. Instead, they emit code that calls methods on whatever type the local `env` binding exposes:

```
env . cost_estimate () . budget () . cpu_instruction_cost ()
└─┬─┘ └─────┬────────┘ └───┬──┘ └─────────┬──────────────┘
  │          │              │                └── Returns u64
  │          │              └── Returns Budget-like type
  │          └── Returns CostEstimate-like type
  └── User-defined binding, typically soroban_sdk::Env
```

In real usage, this chain reads the Soroban VM's internal cost counters. The counters are reset before the measured work by calling `budget.reset_unlimited()`, which the developer writes explicitly in the test body — the macro does not inject it because the reset point depends on which part of the setup should be excluded from measurement.

The macros measure **local estimates**, not network-verified costs. These estimates can differ from real network costs depending on build profile, host function overheads, and protocol parameters. The `cargo-budget-report` CLI (a separate crate) provides the network-verified counterpart.