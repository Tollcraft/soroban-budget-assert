with open("cargo-budget-report/src/main.rs", "r") as f:
    text = f.read()

# Mods conflict
text = text.replace(
"""<<<<<<< HEAD
mod network_guard;
=======
mod json_output;
>>>>>>> origin/main""",
"""mod network_guard;
mod json_output;"""
)

# Exports conflict
import re
text = re.sub(
    r"<<<<<<< HEAD\n\s*let exported_fns: HashSet<String> = match contract_exports::scan_wasm_exports\(&wasm_bytes\)\?\n\s*\{\n\s*contract_exports::ExportScan::Functions\(fns\) => fns.into_iter\(\).collect\(\),\n\s*other => \{\n\s*if let Some\(diagnostic\) = other.diagnostic\(&package.name\) \{\n\s*eprintln!\(\"Error: \{diagnostic\}\"\);\n=======\n.*?\n>>>>>>> origin/main\n\s*\}\n\s*// A crate explicitly built as a cdylib that exports no\n\s*// contract entrypoint is a real misconfiguration: fail the\n\s*// run so CI does not treat it as \"nothing to report\".\n\s*has_errors = true;\n\s*continue;\n\s*\}\n\s*};",
    """        let exported_fns: HashSet<String> = match contract_exports::scan_wasm_exports(&wasm_bytes)?
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
        all_exported.extend(exported_fns.iter().cloned());""",
    text,
    flags=re.DOTALL
)

with open("cargo-budget-report/src/main.rs", "w") as f:
    f.write(text)
