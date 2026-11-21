mod error {
    #[derive(Debug)]
    pub struct Error(String);

    pub type Result<T> = std::result::Result<T, Error>;

    impl std::fmt::Display for Error {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(&self.0)
        }
    }

    impl From<wasmparser::BinaryReaderError> for Error {
        fn from(error: wasmparser::BinaryReaderError) -> Self {
            Self(error.message().to_string())
        }
    }
}

#[path = "../src/contract_exports.rs"]
mod contract_exports;

use contract_exports::{not_a_cdylib_message, scan_wasm_exports, ExportScan};

fn runtime_only_wasm(func_exports: &[&str], include_memory: bool) -> Vec<u8> {
    let mut module = b"\0asm".to_vec();
    module.extend_from_slice(&1u32.to_le_bytes());

    section(&mut module, 1, &[0x01, 0x60, 0x00, 0x00]);
    section(&mut module, 3, &[0x01, 0x00]);
    if include_memory {
        section(&mut module, 5, &[0x01, 0x00, 0x01]);
    }

    section(&mut module, 7, &{
        let mut body = vec![(func_exports.len() + usize::from(include_memory)) as u8];
        for name in func_exports {
            body.push(name.len() as u8);
            body.extend_from_slice(name.as_bytes());
            body.push(0x00);
            body.push(0x00);
        }
        if include_memory {
            body.push(6);
            body.extend_from_slice(b"memory");
            body.push(0x02);
            body.push(0x00);
        }
        body
    });
    section(&mut module, 10, &[0x01, 0x02, 0x00, 0x0b]);
    module
}

fn section(module: &mut Vec<u8>, id: u8, body: &[u8]) {
    module.push(id);
    module.push(body.len() as u8);
    module.extend_from_slice(body);
}

#[test]
fn only_runtime_function_exports_are_classified_as_runtime_symbols() {
    let wasm = runtime_only_wasm(&["__heap_base", "_start"], true);
    let scan = scan_wasm_exports(&wasm).expect("the synthesized WASM must parse");

    assert_eq!(
        scan,
        ExportScan::OnlyRuntimeSymbols {
            found: vec!["__heap_base".to_string(), "_start".to_string()],
        },
        "runtime names must never be promoted to contract entrypoints"
    );
}

#[test]
fn the_runtime_only_diagnostic_names_the_symbols_and_the_fix() {
    let wasm = runtime_only_wasm(&["__heap_base"], true);
    let scan = scan_wasm_exports(&wasm).expect("the synthesized WASM must parse");
    let diagnostic = scan
        .diagnostic("runtime-only")
        .expect("a non-Functions scan must be actionable");

    for expected in [
        "Package 'runtime-only'",
        "Exports found: __heap_base",
        "#[contractimpl]",
    ] {
        assert!(
            diagnostic.contains(expected),
            "expected {expected:?} in: {diagnostic}"
        );
    }

    let cdylib_advice = not_a_cdylib_message("runtime-only");
    assert!(
        cdylib_advice.contains("crate-type = [\"cdylib\"]"),
        "the user guidance must advise compiling as cdylib: {cdylib_advice}"
    );
}

#[test]
fn a_memory_only_export_is_not_a_function_export() {
    let wasm = runtime_only_wasm(&[], true);
    assert_eq!(
        scan_wasm_exports(&wasm).unwrap(),
        ExportScan::NoFunctionExports,
        "`memory` is a memory export, so it cannot be a runtime function export"
    );
}
