use budget_macros::budget_cpu_lt;

// A tuple struct is also rejected with "expected `fn`", verifying that the
// macro's ItemFn parse fails for every struct variant, not just unit structs.
#[budget_cpu_lt(1000)]
struct TupleStruct(u32, String);

fn main() {}
