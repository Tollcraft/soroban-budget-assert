use budget_macros::budget_ledger_entries_lt;

// budget_ledger_entries_lt parses ItemFn directly (not via expand_targets).
// A struct must still be rejected with "expected `fn`".
#[budget_ledger_entries_lt(5)]
struct NotAFunction;

fn main() {}
