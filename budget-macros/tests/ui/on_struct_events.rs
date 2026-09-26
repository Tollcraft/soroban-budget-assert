use budget_macros::budget_events_lt;

// budget_events_lt parses ItemFn directly (not via expand_targets).
// A struct must still be rejected with "expected `fn`".
#[budget_events_lt(10)]
struct NotAFunction;

fn main() {}
