use budget_macros::budget_cpu_lt;

// The macro generates code that references `env.cost_estimate().budget()`.
// This fixture supplies no `env` binding and no `env_ident = ...` override, so
// the generated assertion names an `env` value that does not exist and the
// test binary must fail to compile (E0423). The exact diagnostic is pinned in
// `missing_env.stderr`.
//
// The parser-level contract this relies on — a spec parsed without `env_ident`
// leaves the default `env` identifier for expansion to supply — is covered by
// the `budget_macros::tests` unit tests in `src/lib.rs`.
#[budget_cpu_lt(1000)]
fn test_without_env() {}

fn main() {}
