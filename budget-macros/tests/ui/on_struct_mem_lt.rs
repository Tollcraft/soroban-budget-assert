use budget_macros::budget_mem_lt;

// budget_mem_lt uses the same expand_targets path as budget_cpu_lt, so
// applying it to a struct must also produce "expected `fn`".
#[budget_mem_lt(500)]
struct NotAFunction;

fn main() {}
