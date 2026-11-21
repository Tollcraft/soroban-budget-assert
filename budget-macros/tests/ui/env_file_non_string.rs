//! `env_file` accepts any expression, so two wrong paths compile differently:
//! a non-string literal reaches the generated `let env_file_path: &str = …`
//! binding and rustc rejects the value, while an empty string literal is
//! knowable during expansion and is refused as a missing file. Both carets must
//! name the offending argument rather than the whole attribute.

#[path = "support/mock_env.rs"]
mod mock_env;

use budget_macros::budget_cpu_lt;
use mock_env::Env;

// `env_file` is parsed as an `Expr` here, so the value is embedded into the
// assertion's limit expression and only rustc can tell it is not a `&str`.
#[budget_cpu_lt(env_file = 42, env = "SOME_LIMIT")]
fn env_file_path_is_not_a_string() {
    let env = Env::new(10, 0);
    let _ = env.cost_estimate().budget().cpu_instruction_cost();
}

// A string literal is resolved during expansion; the empty path names no file,
// so the not-found diagnostic has to quote it.
#[budget_cpu_lt(env_file = "", env = "SOME_LIMIT")]
fn env_file_path_is_empty() {}

fn main() {}
