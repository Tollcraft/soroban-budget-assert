//! Minimal stand-in for `soroban_sdk::Env` for the UI tests.
//!
//! The macros only emit `env.cost_estimate().budget().<metric>()` (and, for the
//! event / ledger-entry macros, `env.events()` and `budget.tracker(…)`), so the
//! UI tests can exercise every body shape against this mock instead of pulling
//! in the SDK and compiling a contract. Reported costs are fixed per instance
//! so a test can decide up front whether the injected assertion should pass or
//! panic.

#![allow(dead_code)]

pub struct Env {
    cpu: u64,
    mem: u64,
    events: usize,
    read_entries: u64,
    write_entries: u64,
}

pub struct CostEstimate<'a> {
    env: &'a Env,
}

pub struct Budget<'a> {
    env: &'a Env,
}

impl Env {
    pub fn new(cpu: u64, mem: u64) -> Self {
        Env {
            cpu,
            mem,
            events: 0,
            read_entries: 0,
            write_entries: 0,
        }
    }

    /// Build an `Env` with explicit event and ledger-entry counts, used by the
    /// `budget_events_lt` / `budget_ledger_entries_lt` UI pass tests.
    pub fn new_full(cpu: u64, mem: u64, events: usize, read_entries: u64, write_entries: u64) -> Self {
        Env {
            cpu,
            mem,
            events,
            read_entries,
            write_entries,
        }
    }

    pub fn cost_estimate(&self) -> CostEstimate<'_> {
        CostEstimate { env: self }
    }

    pub fn events(&self) -> Events {
        Events { count: self.events }
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

    pub fn tracker(&self, ct: ContractCostType) -> CostTracker {
        let iterations = match ct {
            ContractCostType::DiskReadEntries => self.env.read_entries,
            ContractCostType::DiskWriteEntries => self.env.write_entries,
        };
        CostTracker { iterations }
    }
}

/// Stand-in for `soroban_sdk::Events`.
pub struct Events {
    count: usize,
}

impl Events {
    pub fn all(&self) -> ContractEvents {
        // The real `ContractEvents::events()` yields `&[xdr::ContractEvent]`; the
        // count is what the macro cares about, so a `Vec<u8>` of the right
        // length is enough for the mock.
        ContractEvents {
            events: vec![0u8; self.count],
        }
    }
}

/// Stand-in for `soroban_sdk::testutils::ContractEvents`.
pub struct ContractEvents {
    events: Vec<u8>,
}

impl ContractEvents {
    pub fn events(&self) -> &[u8] {
        &self.events
    }
}

/// Stand-in for `soroban_sdk::ContractCostType` (only the variants the budget
/// macros read).
#[derive(Clone, Copy)]
pub enum ContractCostType {
    DiskReadEntries,
    DiskWriteEntries,
}

/// Stand-in for `soroban_sdk`'s cost tracker returned by `Budget::tracker`.
pub struct CostTracker {
    iterations: u64,
}

impl CostTracker {
    pub fn iterations(&self) -> u64 {
        self.iterations
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
