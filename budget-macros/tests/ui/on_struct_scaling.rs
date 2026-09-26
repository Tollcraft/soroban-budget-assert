use budget_macros::budget_scaling;

// budget_scaling parses ItemFn directly (not via expand_targets).
// A struct must still be rejected with "expected `fn`".
#[budget_scaling(sizes = [10, 100], model = linear, tolerance = 0.3)]
struct NotAFunction;

fn main() {}
