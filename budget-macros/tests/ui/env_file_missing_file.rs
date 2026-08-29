use budget_macros::budget_cpu_lt;

// A literal `env_file` path that does not resolve to a file must fail at
// compile time, naming the path — not defer to a runtime panic and not
// silently fall through to `u64::MAX`.
#[budget_cpu_lt(env_file = "definitely/not/a/real/limits.env", env = "SOME_LIMIT")]
fn test_missing_env_file() {
    let env = ();
}

fn main() {}
