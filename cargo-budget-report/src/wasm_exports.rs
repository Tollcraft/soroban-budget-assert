//! WASM export-section parsing for `cargo budget-report`.
//!
//! The tool decides which contract functions to simulate by reading the
//! compiled contract's WASM export section: every exported function is a
//! simulation candidate. This module isolates that parse so the exact set of
//! names it yields for a given module is pinned by a test — a `wasmparser`
//! upgrade that quietly changed the parse would otherwise leave the tool
//! silently simulating the wrong functions with no error to attribute it to.

use std::collections::BTreeSet;

use anyhow::Context;
use wasmparser::{ExternalKind, Parser, Payload};

/// Returns the exported **function** names in `wasm_bytes` that name a
/// contract entry point.
///
/// Filtering rule (unchanged across the 0.116 → 0.254 `wasmparser` upgrade):
/// an export is kept when its kind is [`ExternalKind::Func`], its name does
/// not start with `_`, and its name is not `memory`. Non-function exports
/// (memories, globals, tables) are ignored. The result is a [`BTreeSet`], so
/// downstream simulation visits the functions in a deterministic order.
pub fn parse_exported_functions(wasm_bytes: &[u8]) -> anyhow::Result<BTreeSet<String>> {
    let mut exported = BTreeSet::new();

    for payload in Parser::new(0).parse_all(wasm_bytes) {
        let payload = payload.context("failed to parse a WASM payload")?;
        if let Payload::ExportSection(exports) = payload {
            for export in exports {
                let export = export.context("failed to read a WASM export entry")?;
                if export.kind == ExternalKind::Func
                    && !export.name.starts_with('_')
                    && export.name != "memory"
                {
                    exported.insert(export.name.to_string());
                }
            }
        }
    }

    Ok(exported)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hand-assembled module. Exports, in section order: `add` (func),
    /// `sub` (func), `memory` (mem), `_internal` (func). Regenerate with the
    /// `python3` snippet in `tests/fixtures/exports_min.wat`.
    const EXPORTS_MIN: &[u8] = include_bytes!("../tests/fixtures/exports_min.wasm");

    #[test]
    fn keeps_named_function_exports() {
        let got = parse_exported_functions(EXPORTS_MIN).unwrap();
        let want: BTreeSet<String> = ["add", "sub"].iter().map(|s| s.to_string()).collect();
        assert_eq!(got, want);
    }

    #[test]
    fn drops_memory_and_underscore_prefixed_exports() {
        let got = parse_exported_functions(EXPORTS_MIN).unwrap();
        assert!(!got.contains("memory"));
        assert!(!got.contains("_internal"));
    }

    #[test]
    fn empty_module_yields_no_exports() {
        let module = [0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
        assert!(parse_exported_functions(&module).unwrap().is_empty());
    }

    #[test]
    fn garbage_bytes_are_an_error_not_a_silent_empty_set() {
        let err = parse_exported_functions(b"not wasm at all").unwrap_err();
        assert!(err.to_string().contains("WASM"));
    }
}
