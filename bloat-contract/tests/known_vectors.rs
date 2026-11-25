//! Published hash/checksum vectors for the exported bloat-contract routines.

use bloat_contract::{BloatContract, BloatContractClient};
use soroban_sdk::{Bytes, Env};

fn client(env: &Env) -> BloatContractClient<'_> {
    let id = env.register(BloatContract, ());
    BloatContractClient::new(env, &id)
}

#[test]
fn crc32_uses_the_ieee_vector() {
    let env = Env::default();
    let client = client(&env);
    let input = Bytes::from_slice(&env, b"123456789");

    assert_eq!(client.run_crc32(&input), 0xCBF4_3926);
}

#[test]
fn fnv1a_matches_the_published_64_bit_vector_fold() {
    let env = Env::default();
    let client = client(&env);
    let input = Bytes::from_slice(&env, b"123456789");

    // The contract intentionally folds the published FNV-1a 64-bit result
    // into its u32 return type: 0x006d5573923c6dfc -> 0x25139ac5.
    assert_eq!(client.run_fnv1a(&input), 0x2513_9AC5);
}

#[test]
fn djb2_matches_the_known_single_byte_vector() {
    let env = Env::default();
    let client = client(&env);
    let input = Bytes::from_slice(&env, b"a");

    assert_eq!(client.run_djb2(&input), 0x0002_B606);
}

#[test]
fn adler32_matches_the_wikipedia_vector() {
    let env = Env::default();
    let client = client(&env);
    let input = Bytes::from_slice(&env, b"Wikipedia");

    assert_eq!(client.run_adler32(&input), 0x11E6_0398);
}
