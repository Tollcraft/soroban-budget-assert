use budget_macros::budget_cpu_lt;

struct Contract;

// An impl block the macro can parse but that carries no methods to
// instrument must fail loudly rather than expand to nothing.
//
// The diagnostic is emitted by the shared impl-block walker whenever zero
// methods end up instrumented — either the block has no functions at all, or
// every function already carries its own budget attribute. Both cases are
// pinned by the `budget_macros::tests` unit tests in `src/lib.rs`.
#[budget_cpu_lt(1_000)]
impl Contract {}

fn main() {}