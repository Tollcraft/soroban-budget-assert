//! Minimal stand-in for `soroban_sdk::Env` for the UI tests.
//!
//! The macros only emit `env.cost_estimate().budget().<metric>()`, so the UI
//! tests can exercise every body shape against this mock instead of pulling in
//! the SDK and compiling a contract. Reported costs are fixed per instance so a
//! test can decide up front whether the injected assertion should pass or panic.

#![allow(dead_code)]

pub struct Env {
    cpu: u64,
    mem: u64,
}

pub struct CostEstimate<'a> {
    env: &'a Env,
}

pub struct Budget<'a> {
    env: &'a Env,
}

impl Env {
    pub fn new(cpu: u64, mem: u64) -> Self {
        Env { cpu, mem }
    }

    pub fn cost_estimate(&self) -> CostEstimate<'_> {
        CostEstimate { env: self }
    }
}

impl<'a> CostEstimate<'a> {
    pub fn budget(&self) -> Budget<'a> {
        Budget { env: self.env }
    }
}

impl Budget<'_> {
    pub fn cpu_instruction_cost(&self) -> u64 {
        self.env.cpu
    }

    pub fn memory_bytes_cost(&self) -> u64 {
        self.env.mem
    }
}

/// Runs `f`, returning the budget assertion's panic message if it panicked.
pub fn budget_panic<F: FnOnce() -> R + std::panic::UnwindSafe, R>(f: F) -> Option<String> {
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let result = std::panic::catch_unwind(f);
    std::panic::set_hook(previous_hook);

    match result {
        Ok(_) => None,
        Err(payload) => Some(
            payload
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default(),
        ),
    }
}
