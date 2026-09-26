use budget_macros::budget_cpu_lt;

// The macro must reject a struct even when the limit is a percentage form.
// Using `of = env = "..."` instead of `env_file` to avoid the
// expansion-time file resolution check, which would mask the struct error.
#[budget_cpu_lt(pct = 50, of = env = "SOME_NETWORK_LIMIT")]
struct NotAFunction;

fn main() {}
