use budget_macros::budget_write_bytes_lt;

// budget_write_bytes_lt uses expand_targets. A struct must fail with the
// same "expected `fn`" error.
#[budget_write_bytes_lt(200)]
struct NotAFunction;

fn main() {}
