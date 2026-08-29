import re

# Resolve docs/src/reference.md
with open("docs/src/reference.md", "r") as f:
    text = f.read()

text = re.sub(
    r"<<<<<<< HEAD\n(.*?)\n=======\n.*?\n>>>>>>> origin/main\n",
    r"\1\n",
    text,
    flags=re.DOTALL
)

with open("docs/src/reference.md", "w") as f:
    f.write(text)

# Resolve cargo-budget-report/src/main.rs
with open("cargo-budget-report/src/main.rs", "r") as f:
    text = f.read()

# First conflict (mods):
text = text.replace(
"""<<<<<<< HEAD
mod network_guard;
=======
mod json_output;
>>>>>>> origin/main""",
"""mod network_guard;
mod json_output;"""
)

# Second conflict (exports):
replace_target = """<<<<<<< HEAD
        let exported_fns: HashSet<String> = match contract_exports::scan_wasm_exports(&wasm_bytes)?
        {
            contract_exports::ExportScan::Functions(fns) => fns.into_iter().collect(),
            other => {
                if let Some(diagnostic) = other.diagnostic(&package.name) {
                    eprintln!("Error: {diagnostic}");
                }
                // A crate explicitly built as a cdylib that exports no
                // contract entrypoint is a real misconfiguration: fail the
                // run so CI does not treat it as "nothing to report".
                has_errors = true;
                continue;
            }
        };
=======
        for payload in WasmParser::new(0).parse_all(&wasm_bytes) {
            if let wasmparser::Payload::ExportSection(export_section) = payload? {
                for export_item in export_section {
                    let export_item = export_item?;
                    if export_item.kind == wasmparser::ExternalKind::Func {
                        let name = export_item.name.to_string();
                        // Ignore internal and common exports
                        if !name.starts_with('_') && name != "memory" {
                            exported_fns.insert(name.clone());
                            all_exported.insert(name);
                        }
                    }
>>>>>>> origin/main
                }
                // A crate explicitly built as a cdylib that exports no
                // contract entrypoint is a real misconfiguration: fail the
                // run so CI does not treat it as "nothing to report".
                has_errors = true;
                continue;
            }
        };"""

replacement = """        let exported_fns: HashSet<String> = match contract_exports::scan_wasm_exports(&wasm_bytes)?
        {
            contract_exports::ExportScan::Functions(fns) => fns.into_iter().collect(),
            other => {
                if let Some(diagnostic) = other.diagnostic(&package.name) {
                    eprintln!("Error: {diagnostic}");
                }
                // A crate explicitly built as a cdylib that exports no
                // contract entrypoint is a real misconfiguration: fail the
                // run so CI does not treat it as "nothing to report".
                has_errors = true;
                continue;
            }
        };
        all_exported.extend(exported_fns.iter().cloned());"""

text = text.replace(replace_target, replacement)

with open("cargo-budget-report/src/main.rs", "w") as f:
    f.write(text)

