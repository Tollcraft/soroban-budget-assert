use budget_macros::budget_cpu_lt;

// A struct with named fields must also be rejected. The ItemFn parser fails
// the same way regardless of the struct's internal shape.
#[budget_cpu_lt(1000)]
struct NamedFields {
    x: i32,
    y: String,
}

fn main() {}
