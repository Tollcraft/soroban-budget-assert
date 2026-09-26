use budget_macros::budget_cpu_lt;

// The macro must reject a struct even when a baseline is specified.
// The error is about the struct, not the baseline expression.
#[budget_cpu_lt(1000, baseline = 100)]
struct NotAFunction;

fn main() {}
