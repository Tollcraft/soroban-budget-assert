//! Compile-time behavior of the budget macros.
//!
//! `tests/ui/*.rs` must fail to compile, with the diagnostic pinned in the
//! matching `.stderr`. `tests/ui/pass/*.rs` must compile *and run*: those cases
//! exercise each supported test-body shape against the mock `env` in
//! `tests/ui/support/mock_env.rs` and assert which cost and limit the injected
//! check reports, so a body shape that silently stops being checked fails here.
//!
//! Regenerate the `.stderr` snapshots with `TRYBUILD=overwrite cargo test -p budget-macros`.

#[test]
fn ui_tests() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
    t.pass("tests/ui/pass/*.rs");
}
