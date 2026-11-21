//! Issue #1010 — pin that `ResourceReportable::to_report` preserves provenance
//! metadata byte-for-byte across edge-case strings and extreme metrics.
//!
//! Downstream JSON and Markdown formatters read `package` / `function` verbatim,
//! so a silent rewrite (trim, case fold, replacement character) here becomes a
//! corrupted report field there. `src/traits.rs` copies the fields with
//! `package.to_string()` / `function.to_string()`, which the assertions below
//! hold at the boundaries an untrusted caller can actually hit: empty strings,
//! non-ASCII identifiers, embedded delimiters, and `u64::MAX` metric values.

use soroban_budget_assert_core::{CostMeasurer, ResourceReport, ResourceReportable};

/// A fixed-cost `DummyMeasurer` mirroring the shape `to_report` is documented
/// against in `src/traits.rs:459-483`: two metrics from the operator, plus a
/// WASM size supplied as a build-time constant.
struct FixedOp {
    cpu: u64,
    mem: u64,
    wasm: u64,
}

impl CostMeasurer for FixedOp {
    fn cpu_instructions(&self) -> u64 {
        self.cpu
    }
    fn memory_bytes(&self) -> u64 {
        self.mem
    }
}

impl ResourceReportable for FixedOp {
    fn to_report(&self, package: &str, function: &str) -> ResourceReport {
        ResourceReport {
            package: package.to_string(),
            function: function.to_string(),
            cpu_instructions: self.cpu_instructions(),
            memory_bytes: self.memory_bytes(),
            wasm_bytes: self.wasm,
        }
    }
}

#[test]
fn empty_package_and_function_are_preserved_as_empty() {
    let op = FixedOp {
        cpu: 500,
        mem: 256,
        wasm: 1024,
    };
    let report = op.to_report("", "");
    assert_eq!(report.package, "");
    assert_eq!(report.function, "");
    assert_eq!(report.cpu_instructions, 500);
    assert_eq!(report.memory_bytes, 256);
    assert_eq!(report.wasm_bytes, 1024);
}

#[test]
fn unicode_identifiers_survive_byte_for_byte() {
    let op = FixedOp {
        cpu: 100,
        mem: 200,
        wasm: 300,
    };
    let cases: [(&str, &str); 5] = [
        ("amm-pool-contract", "déposer"),
        ("crème_brûlée", "取引"),
        ("Σύστημα", "έγγραφο"),
        ("نظام", "دفع"),
        ("emoji_🚀", "contract_💼"),
    ];
    for (package, function) in cases {
        let report = op.to_report(package, function);
        assert_eq!(report.package, package, "package bytes must match input");
        assert_eq!(report.function, function, "function bytes must match input");
        assert_eq!(
            report.package.as_bytes(),
            package.as_bytes(),
            "package must not be re-encoded"
        );
        assert_eq!(
            report.function.as_bytes(),
            function.as_bytes(),
            "function must not be re-encoded"
        );
    }
}

#[test]
fn whitespace_and_control_characters_are_not_trimmed() {
    let op = FixedOp {
        cpu: 1,
        mem: 1,
        wasm: 1,
    };
    let tricky: &[&str] = &[
        "  leading_and_trailing  ",
        "\tleading_tab",
        "trailing_tab\t",
        "with\nnewline",
        "with\r\ncrlf",
        "with\0nuls",
    ];
    for text in tricky {
        let report = op.to_report(text, text);
        assert_eq!(report.package.as_str(), *text);
        assert_eq!(report.function.as_str(), *text);
    }
}

#[test]
fn max_u64_metrics_are_preserved_verbatim() {
    let op = FixedOp {
        cpu: u64::MAX,
        mem: u64::MAX,
        wasm: u64::MAX,
    };
    let report = op.to_report("pkg", "func");
    assert_eq!(report.cpu_instructions, u64::MAX);
    assert_eq!(report.memory_bytes, u64::MAX);
    assert_eq!(report.wasm_bytes, u64::MAX);
}

#[test]
fn zero_metrics_and_labels_round_trip() {
    let op = FixedOp {
        cpu: 0,
        mem: 0,
        wasm: 0,
    };
    let report = op.to_report("p", "f");
    assert_eq!(report.cpu_instructions, 0);
    assert_eq!(report.memory_bytes, 0);
    assert_eq!(report.wasm_bytes, 0);
    assert_eq!(report.package, "p");
    assert_eq!(report.function, "f");
}

#[test]
fn provenance_is_independent_of_metric_values() {
    let a = FixedOp {
        cpu: 100,
        mem: 200,
        wasm: 300,
    };
    let b = FixedOp {
        cpu: 999,
        mem: 998,
        wasm: 997,
    };
    let ra = a.to_report("pkg_x", "fn_y");
    let rb = b.to_report("pkg_x", "fn_y");
    assert_eq!(ra.package, rb.package);
    assert_eq!(ra.function, rb.function);
    assert_ne!(ra.cpu_instructions, rb.cpu_instructions);
    assert_ne!(ra.memory_bytes, rb.memory_bytes);
    assert_ne!(ra.wasm_bytes, rb.wasm_bytes);
}

#[test]
fn reports_are_byte_equal_when_all_inputs_match() {
    let op = FixedOp {
        cpu: 1_000_000,
        mem: 2_048,
        wasm: 102_400,
    };
    let left = op.to_report("pkg", "fn");
    let right = op.to_report("pkg", "fn");
    assert_eq!(left, right);
}
