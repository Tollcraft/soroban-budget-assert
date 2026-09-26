use budget_macros::budget_cpu_lt;

// The macro must reject a struct even when the limit is an environment
// variable rather than an integer literal. The error must be about the
// struct (not the limit form), proving the fn-parse happens before any
// limit-source evaluation.
#[budget_cpu_lt(env = "SOME_LIMIT")]
struct NotAFunction;

fn main() {}
