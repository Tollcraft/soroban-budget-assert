use budget_macros::budget_cpu_lt;

struct Contract;

// An impl block the macro can parse but that carries no methods to
// instrument must fail loudly rather than expand to nothing.
#[budget_cpu_lt(1_000)]
impl Contract {}

fn main() {}
