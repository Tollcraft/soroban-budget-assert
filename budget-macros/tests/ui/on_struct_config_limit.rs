use budget_macros::budget_cpu_lt;

// The macro must reject a struct even when the limit is a config key.
// The error must be about the struct, not the config source.
#[budget_cpu_lt(config = "some_key")]
struct NotAFunction;

fn main() {}
