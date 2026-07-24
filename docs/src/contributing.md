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

* **Supported SDK Version**: `soroban-sdk` = `"22.0.0"` (specifically tested/resolved to `22.0.11` in `Cargo.lock`)
* **Supported XDR Version**: `stellar-xdr` = `"22.1.0"` (used for decoding transaction simulation responses)
* **Corresponding Stellar Protocol**: **Protocol 22**

### Compatibility Matrix

| SDK Version | Protocol Version | Status | Notes |
| :--- | :--- | :--- | :--- |
| **`< 22.0.0`** | `< 22` | **Untested** | Older protocols may use different transaction/resource schemas. |
| **`22.0.x`** | `22` | **Supported** | Matches pinned manifest dependencies (`soroban-sdk` `22.0.0`, `stellar-xdr` `22.1.0`). |
| **`>= 23.0.0`** | `>= 23` | **Untested** | Future protocol upgrades or XDR schema changes (e.g. key/field renames) may break parsing. |
