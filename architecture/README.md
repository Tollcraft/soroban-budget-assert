# Architecture Diagrams

Visual reference for the two primary subsystems in `soroban-budget-assert`.
Both diagrams are authored in [Mermaid](https://mermaid.js.org/).

**Note on Editing:** These diagrams are embedded directly in the Markdown pages of the documentation site so they render live without extra plugins. To edit them, modify the Mermaid source blocks directly in `docs/src/mechanics.md` and `docs/src/macro_architecture.md`. The `.mmd` files in this directory are kept as standalone backups and history references.

---

## Diagram 1 — `cargo-budget-report` Pipeline

Shows the end-to-end lifecycle from the developer running `cargo budget-report`
through WASM compilation, export discovery, RPC simulation, metric extraction,
and final report generation. The `--check` path is highlighted to show how CI
enforcement works.

```mermaid
sequenceDiagram
    autonumber
    participant Dev as Developer
    participant CLI as cargo budget-report
    participant Cargo as Cargo / rustc
    participant WASM as WASM Binary
    participant Parser as WASM Parser
    participant Stellar as Stellar CLI
    participant RPC as Soroban RPC
    participant Report as Budget Report

    Dev->>CLI: cargo budget-report [--check] [--json]
    CLI->>Cargo: cargo build -p <contract> --release<br/>--target wasm32v1-none
    Cargo-->>WASM: amm_pool_contract.wasm (cdylib)

    CLI->>Parser: wasmparser: scan exports
    Parser-->>CLI: exported function names list

    CLI->>CLI: Read budget.toml<br/>(network, source, per-function args/limits)

    loop For each exported function in budget.toml
        CLI->>Stellar: stellar contract invoke<br/>--function <fn> --args <args>
        Stellar->>RPC: simulateTransaction (XDR)
        RPC-->>Stellar: SimulateTransactionResponse<br/>(transactionData XDR)
        Stellar-->>CLI: stdout JSON response

        CLI->>CLI: Decode transactionData XDR<br/>Extract: CPU instructions,<br/>read bytes, write bytes

        alt --check mode and limit configured
            CLI->>CLI: Compare value vs cpu_limit /<br/>read_limit / write_limit
            CLI-->>Report: PASS or FAIL per metric
        else report-only mode
            CLI-->>Report: Record value (no enforcement)
        end
    end

    CLI->>Report: Render table (tabled) or<br/>emit JSON (--json flag)
    Report-->>Dev: Console output +<br/>current_report.json artifact

    alt Any limit breached or simulation failed
        CLI-->>Dev: exit code 1 (CI fails)
    else All checks pass
        CLI-->>Dev: exit code 0 (CI passes)
    end
```

**Source file:** [`cargo-budget-report-pipeline.mmd`](./cargo-budget-report-pipeline.mmd)

---

## Diagram 2 — `#[budget_cpu_lt]` Macro Compile-Time Expansion

Shows how the `budget_cpu_lt` procedural macro rewrites a test function at
compile time. Covers both the static integer form (`#[budget_cpu_lt(N)]`) and
the dynamic environment-variable form (`#[budget_cpu_lt(env = "VAR")]`), and
traces the two runtime outcomes: passing within the limit and panicking on a
regression.

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

**Source file:** [`budget-cpu-lt-macro-expansion.mmd`](./budget-cpu-lt-macro-expansion.mmd)

---

## Files in this directory

| File | Description |
|------|-------------|
| `cargo-budget-report-pipeline.mmd` | Diagram 1 — full `cargo budget-report` lifecycle |
| `budget-cpu-lt-macro-expansion.mmd` | Diagram 2 — `#[budget_cpu_lt]` compile-time expansion |
| `README.md` | This file — rendered entry point for GitHub and GitBook |
