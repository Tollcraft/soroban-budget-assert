//! Pass fixture for `#[budget_scaling]` - exercises a scaling attribute on a
//! real function whose cost grows with the size parameter.
//!
//! The macro creates its own `soroban_sdk::Env` per iteration, so the `env`
//! parameter in the function signature is ignored (shadowed by the macro's
//! own Env). The `size` parameter is bound to the current iteration value by
//! the macro (via `let size = size;`), so the parameter appears unused but is
//! actually used via the macro's binding.
//!
//! The generated functions are public (preserving original visibility) so we
//! can call them directly from main to verify they work.

use budget_macros::budget_scaling;
use soroban_sdk::{Env, Bytes};

/// A simple function that does work proportional to `size` using SHA-256.
/// The cost grows linearly with the input size.
#[budget_scaling(sizes = [100, 1000, 10000], model = linear, tolerance = 1.0)]
pub fn operation_scales_linearly(_env: Env, _size: u32) {
    // Use the macro's `env` (shadows the parameter) to hash data of `size` bytes.
    // This incurs measurable CPU cost in the Soroban test Env.
    // The `size` parameter is bound by the macro's `let size = size;`
    let data = Bytes::from_slice(&env, &vec![0u8; size as usize]);
    let _hash = env.crypto().sha256(&data);
}

/// A function with quadratic cost growth (hashing size^2 bytes).
#[budget_scaling(sizes = [10, 20, 40], model = quadratic, tolerance = 1.0)]
pub fn operation_scales_quadratically(_env: Env, _size: u32) {
    // Hash size^2 bytes for quadratic cost
    // The `size` parameter is bound by the macro's `let size = size;`
    let total_bytes = (size * size) as usize;
    let data = Bytes::from_slice(&env, &vec![0u8; total_bytes]);
    let _hash = env.crypto().sha256(&data);
}

/// A function that should FAIL the scaling check (flat cost across sizes).
/// This tests that a breach is actually caught.
#[budget_scaling(sizes = [100, 1000, 10000], model = linear, tolerance = 0.1)]
pub fn operation_should_fail_flat_cost(_env: Env, _size: u32) {
    // Constant cost regardless of size - should fail linear scaling check
    // Hash a fixed small amount of data
    let data = Bytes::from_slice(&env, &[0u8; 100]);
    let _hash = env.crypto().sha256(&data);
}

fn main() {
    // The generated test functions are public (preserving original visibility)
    // so we can call them directly from main to verify they work.
    
    // These calls will execute the generated test functions and verify
    // the scaling checks pass.
    operation_scales_linearly(Env::default(), 100);
    operation_scales_quadratically(Env::default(), 10);
    
    // This should panic due to scaling check failure
    let result = std::panic::catch_unwind(|| {
        operation_should_fail_flat_cost(Env::default(), 100);
    });
    assert!(result.is_err(), "expected scaling check to fail for flat cost");
    
    println!("All scaling tests passed!");
}