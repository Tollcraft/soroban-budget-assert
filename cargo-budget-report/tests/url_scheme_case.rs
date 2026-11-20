#[path = "../src/url_checks.rs"]
mod url_checks;

use url_checks::{is_github_repo_url, is_stellar_docs_url};

#[test]
fn test_uppercase_and_mixed_case_scheme_accepted() {
    assert!(is_github_repo_url(
        "HTTPS://github.com/Tollcraft/soroban-budget-assert"
    ));
    assert!(is_github_repo_url(
        "Https://github.com/Tollcraft/soroban-budget-assert"
    ));
    assert!(is_stellar_docs_url("HTTPS://developers.stellar.org/docs"));
    assert!(is_stellar_docs_url("HttpS://developers.stellar.org/docs"));
}

#[test]
fn test_mixed_case_host_accepted() {
    assert!(is_github_repo_url(
        "https://GitHub.com/Tollcraft/soroban-budget-assert"
    ));
    assert!(is_github_repo_url(
        "HTTPS://GITHUB.COM/Tollcraft/soroban-budget-assert"
    ));
    assert!(is_stellar_docs_url("https://Developers.Stellar.Org/docs"));
}

#[test]
fn test_path_casing_preserved() {
    assert!(is_github_repo_url("HTTPS://github.com/Tollcraft/repo"));
    assert!(!is_github_repo_url("HTTPS://github.com/tollcraft/repo"));
}
