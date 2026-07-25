use budget_macros::budget_cpu_lt;

// The macro expects a function item. Applying it to a struct should fail.
#[budget_cpu_lt(1000)]
struct NotAFunction;

fn main() {}
