# Contributing Guide

We welcome contributions to `soroban-budget-assert`. 

### Workflow
1. Fork the repository.
2. Create a feature branch.
3. Commit your changes using conventional commits (e.g., `feat(macro): add new limit`).
4. Push to your fork and submit a Pull Request.

### Requirements
- All new CLI features must support the `--json` flag.
- Macro changes must include corresponding `#[test]` cases in the `amm-pool-contract`.
- Do not introduce panics in the CLI; use `anyhow::Result` for graceful error handling.

## ⚙️ Supported Versions & Compatibility

* **Supported SDK Version**: `soroban-sdk` = `"27.0.3"` (specifically tested/resolved to `27.0.6` in `Cargo.lock`)
* **Supported XDR Version**: `stellar-xdr` = `"27.0.0"` (used for decoding transaction simulation responses)
* **Corresponding Stellar Protocol**: **Protocol 27**

### Compatibility Matrix

| SDK Version | Protocol Version | Status | Notes |
| :--- | :--- | :--- | :--- |
| **`< 22.0.0`** | `< 22` | **Untested** | Older protocols may use different transaction/resource schemas. |
| **`22.0.x`** | `22` | **Untested** | Previously supported; superseded by SDK 27 workspace baseline. |
| **`23.0.x` – `26.0.x`** | `23` – `26` | **Untested** | Not pinned in the workspace; may work but are untested. |
| **`27.0.x`** | `27` | **Supported** | Matches pinned manifest dependencies (`soroban-sdk` `27.0.3`, `stellar-xdr` `27.0.0`). |
