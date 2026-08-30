use budget_macros::budget_read_bytes_lt;

// `budget_read_bytes_lt` with no argument must be rejected the same way its
// siblings are: the limit parser has nothing to read.
#[budget_read_bytes_lt]
fn test_read_bytes_no_arg() {
    let env = ();
}

fn main() {}
