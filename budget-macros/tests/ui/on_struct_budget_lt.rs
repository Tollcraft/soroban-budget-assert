use budget_macros::budget_lt;

// budget_lt uses expand_targets internally. A struct must be rejected with
// "expected `fn`" just like the single-metric macros.
#[budget_lt(cpu = 1000, mem = 500)]
struct NotAFunction;

fn main() {}
