#[path = "../src/deploy_diagnostics.rs"]
mod deploy_diagnostics;

use deploy_diagnostics::{classify, DeployFailureClass};

#[test]
fn test_unfunded_account_outranks_retry_signals() {
    let cases = [
        "error: account is underfunded; please try again later (HTTP 429 rate limit)",
        "txInsufficientBalance: temporary connection timeout, please retry",
        "account not found on network: 503 Service Unavailable, retrying",
        "could not find account: server 500 error occurred, try again",
        "account requires a minimum balance: connection reset by peer",
        "not funded: network is unreachable, timeout occurred",
    ];

    for case in cases {
        assert_eq!(
            classify(case),
            DeployFailureClass::AccountUnfunded,
            "failed on error string: {case}"
        );
    }
}
