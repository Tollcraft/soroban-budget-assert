//! Classify why a contract WASM yielded nothing to simulate.
//!
//! The tool discovers what to simulate by parsing a compiled contract's WASM
//! export section. When that yields nothing usable there are three distinct
//! causes, and the difference is exactly what a new user needs to know:
//!
//! * the crate is not a `cdylib`, so no WASM is produced at all;
//! * the WASM has no function exports — a plain library, or the `#[contract]`
//!   / `#[contractimpl]` macros were never applied;
//! * the WASM exports functions, but every one is a toolchain symbol
//!   (`memory`, `__data_end`, …) rather than a contract entrypoint.
//!
//! Each gets its own message naming the package and saying what to change.

use crate::error::Result;
use std::collections::BTreeSet;

/// True for an exported name that belongs to the toolchain / WASM runtime
/// rather than the Soroban contract calling convention.
fn is_runtime_symbol(name: &str) -> bool {
    name.starts_with('_')
        || matches!(
            name,
            "memory" | "__heap_base" | "__data_end" | "__stack_pointer" | "__rust_alloc"
        )
}

/// Outcome of scanning a contract WASM's export section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportScan {
    /// One or more contract entrypoints were found.
    Functions(BTreeSet<String>),
    /// The export section has no function exports at all. The build most
    /// likely produced a plain library, or the contract macros were never
    /// applied.
    NoFunctionExports,
    /// Function exports exist, but every one is a toolchain / runtime
    /// symbol. `found` lists them so the mismatch is visible.
    OnlyRuntimeSymbols { found: Vec<String> },
}

/// Scan a compiled contract WASM and classify its function exports.
pub fn scan_wasm_exports(wasm: &[u8]) -> Result<ExportScan> {
    let mut all_func_exports: Vec<String> = Vec::new();
    let mut contract_fns: BTreeSet<String> = BTreeSet::new();

    for payload in wasmparser::Parser::new(0).parse_all(wasm) {
        if let wasmparser::Payload::ExportSection(section) = payload? {
            for export in section {
                let export = export?;
                if export.kind == wasmparser::ExternalKind::Func {
                    let name = export.name.to_string();
                    all_func_exports.push(name.clone());
                    if !is_runtime_symbol(&name) {
                        contract_fns.insert(name);
                    }
                }
            }
        }
    }

    if !contract_fns.is_empty() {
        Ok(ExportScan::Functions(contract_fns))
    } else if all_func_exports.is_empty() {
        Ok(ExportScan::NoFunctionExports)
    } else {
        all_func_exports.sort();
        all_func_exports.dedup();
        Ok(ExportScan::OnlyRuntimeSymbols {
            found: all_func_exports,
        })
    }
}

impl ExportScan {
    /// Diagnostic for a scan that produced nothing simulatable.
    ///
    /// Returns `None` for [`ExportScan::Functions`] — callers match that arm
    /// first and never ask for a diagnostic in the success case.
    pub fn diagnostic(&self, package: &str) -> Option<String> {
        match self {
            ExportScan::Functions(_) => None,
            ExportScan::NoFunctionExports => Some(format!(
                "Package '{package}' built a WASM with no function exports.\n  \
                 The crate most likely compiled as a plain library, or the Soroban \
                 contract macros (#[contract] / #[contractimpl]) were never applied.\n  \
                 Put #[contractimpl] on the contract's impl block and confirm its \
                 `[lib] crate-type` includes `cdylib`, then rebuild."
            )),
            ExportScan::OnlyRuntimeSymbols { found } => Some(format!(
                "Package '{package}' exports functions, but none match the Soroban \
                 contract calling convention.\n  Exports found: {}\n  \
                 Those are toolchain symbols, not contract entrypoints. Add \
                 #[contractimpl] to the contract's impl block so its methods are \
                 exported under their own names, then rebuild.",
                found.join(", ")
            )),
        }
    }
}

/// Message for a workspace crate that depends on `soroban-sdk` but is not
/// built as a `cdylib`, so no WASM is produced and it is skipped entirely.
pub fn not_a_cdylib_message(package: &str) -> String {
    format!(
        "Package '{package}' depends on soroban-sdk but its `[lib] crate-type` \
         does not include `cdylib`, so it produces no WASM and cannot be measured. \
         Skipping.\n  Add `crate-type = [\"cdylib\"]` to its `[lib]` section (keep \
         `rlib` too if other crates depend on it) if it is meant to be a contract."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Assemble a minimal valid WASM module exporting the given
    /// `(name, kind)` pairs. `kind` is `0x00` for a function export, so
    /// every export here needs a function to point at; we declare one
    /// no-op function and point every function export at index 0.
    fn wasm_with_exports(func_exports: &[&str], include_memory: bool) -> Vec<u8> {
        let mut module = Vec::new();
        module.extend_from_slice(b"\0asm");
        module.extend_from_slice(&1u32.to_le_bytes());

        // Type section: one `() -> ()` type.
        section(&mut module, 1, &{
            let mut s = vec![0x01]; // one type
            s.extend_from_slice(&[0x60, 0x00, 0x00]); // func: [] -> []
            s
        });

        // Function section: one function of type 0.
        section(&mut module, 3, &[0x01, 0x00]);

        // Memory section: one memory (needed if we export "memory").
        if include_memory {
            section(&mut module, 5, &[0x01, 0x00, 0x01]);
        }

        // Export section.
        section(&mut module, 7, &{
            let total = func_exports.len() + usize::from(include_memory);
            let mut s = vec![total as u8];
            for name in func_exports {
                s.push(name.len() as u8);
                s.extend_from_slice(name.as_bytes());
                s.push(0x00); // func export
                s.push(0x00); // func index 0
            }
            if include_memory {
                s.push(b"memory".len() as u8);
                s.extend_from_slice(b"memory");
                s.push(0x02); // memory export
                s.push(0x00); // memory index 0
            }
            s
        });

        // Code section: one empty function body (just `end`).
        section(&mut module, 10, &[0x01, 0x02, 0x00, 0x0b]);

        module
    }

    fn section(module: &mut Vec<u8>, id: u8, body: &[u8]) {
        module.push(id);
        module.push(body.len() as u8);
        module.extend_from_slice(body);
    }

    #[test]
    fn contract_functions_are_collected() {
        let wasm = wasm_with_exports(&["deposit", "withdraw", "swap"], true);
        match scan_wasm_exports(&wasm).unwrap() {
            ExportScan::Functions(fns) => {
                assert_eq!(
                    fns.into_iter().collect::<Vec<_>>(),
                    vec!["deposit", "swap", "withdraw"]
                );
            }
            other => panic!("expected Functions, got {other:?}"),
        }
    }

    #[test]
    fn no_function_exports_is_its_own_case() {
        let wasm = wasm_with_exports(&[], true);
        assert_eq!(
            scan_wasm_exports(&wasm).unwrap(),
            ExportScan::NoFunctionExports
        );
    }

    #[test]
    fn only_runtime_symbols_lists_what_was_found() {
        let wasm = wasm_with_exports(&["__data_end", "_start"], true);
        match scan_wasm_exports(&wasm).unwrap() {
            ExportScan::OnlyRuntimeSymbols { found } => {
                assert!(found.contains(&"__data_end".to_string()));
                assert!(found.contains(&"_start".to_string()));
            }
            other => panic!("expected OnlyRuntimeSymbols, got {other:?}"),
        }
    }

    #[test]
    fn diagnostics_name_the_package_and_say_what_to_do() {
        let no_exports = ExportScan::NoFunctionExports
            .diagnostic("my-contract")
            .unwrap();
        assert!(no_exports.contains("my-contract"));
        assert!(no_exports.contains("#[contractimpl]"));

        let runtime = ExportScan::OnlyRuntimeSymbols {
            found: vec!["memory".into(), "__data_end".into()],
        }
        .diagnostic("my-contract")
        .unwrap();
        assert!(runtime.contains("my-contract"));
        assert!(
            runtime.contains("__data_end"),
            "lists what was found: {runtime}"
        );

        assert!(ExportScan::Functions(BTreeSet::new())
            .diagnostic("x")
            .is_none());
    }

    #[test]
    fn not_a_cdylib_message_is_actionable() {
        let msg = not_a_cdylib_message("shared-types");
        assert!(msg.contains("shared-types"));
        assert!(msg.contains("cdylib"));
    }
}
