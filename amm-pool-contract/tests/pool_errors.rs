//! Integration coverage for the AMM pool's public error paths.

use amm_pool_contract::{ConstantProductPool, ConstantProductPoolClient, Error};
use soroban_sdk::{testutils::Address as _, Address, Env};

fn client<'a>(env: &'a Env) -> (ConstantProductPoolClient<'a>, Address) {
    let id = env.register(ConstantProductPool, ());
    let client = ConstantProductPoolClient::new(env, &id);
    let user = Address::generate(env);
    env.mock_all_auths();
    (client, user)
}

#[test]
fn initializing_an_initialized_pool_returns_already_initialized() {
    let env = Env::default();
    let (client, _user) = client(&env);

    assert_eq!(client.initialize(), Ok(()));
    assert_eq!(client.initialize(), Err(Error::AlreadyInitialized));
}

#[test]
fn swap_below_minimum_output_returns_slippage_exceeded() {
    let env = Env::default();
    let (client, user) = client(&env);
    client.initialize().unwrap();
    client.deposit(&user, &1_000, &1_000);

    assert_eq!(
        client.swap(&user, &true, &100, &100),
        Err(Error::SlippageExceeded)
    );
}

#[test]
fn empty_pool_swap_is_rejected_by_the_minimum_output() {
    let env = Env::default();
    let (client, user) = client(&env);
    client.initialize().unwrap();

    // With no liquidity, the calculated output is zero. A positive minimum
    // makes the failure explicit through the contract's public error API.
    assert_eq!(
        client.swap(&user, &true, &100, &1),
        Err(Error::SlippageExceeded)
    );
}
