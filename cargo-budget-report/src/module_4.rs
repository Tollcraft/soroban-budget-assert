#![allow(dead_code)]

fn check_url_scheme_host(url: &str, expected_scheme: &str, expected_host: &str) -> bool {
    let trimmed = url.trim();
    if !trimmed.starts_with(expected_scheme) {
        return false;
    }
    let after_scheme = &trimmed[expected_scheme.len()..];
    if !after_scheme.starts_with("://") {
        return false;
    }
    let after_protocol = &after_scheme[3..];
    let host_end = after_protocol.find('/').unwrap_or(after_protocol.len());
    let host = &after_protocol[..host_end];
    host == expected_host
}

pub fn is_github_repo_url(url: &str) -> bool {
    check_url_scheme_host(url, "https", "github.com")
        && url.contains("/Tollcraft/")
}

pub fn is_stellar_docs_url(url: &str) -> bool {
    check_url_scheme_host(url, "https", "developers.stellar.org")
}

pub fn is_stellar_github_url(url: &str) -> bool {
    check_url_scheme_host(url, "https", "github.com")
        && url.contains("/stellar/")
}

pub fn is_tollcraft_docs_url(url: &str) -> bool {
    check_url_scheme_host(url, "https", "tollcraft.gitbook.io")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_repo_url_valid() {
        assert!(is_github_repo_url(
            "https://github.com/Tollcraft/soroban-budget-assert"
        ));
    }

    #[test]
    fn github_repo_url_wrong_host() {
        assert!(!is_github_repo_url(
            "https://gitlab.com/Tollcraft/soroban-budget-assert"
        ));
    }

    #[test]
    fn github_repo_url_missing_org() {
        assert!(!is_github_repo_url(
            "https://github.com/other-org/repo"
        ));
    }

    #[test]
    fn stellar_docs_url_valid() {
        assert!(is_stellar_docs_url(
            "https://developers.stellar.org/docs/learn/fundamentals/fees"
        ));
    }

    #[test]
    fn stellar_docs_url_wrong_domain() {
        assert!(!is_stellar_docs_url(
            "https://stellar.org/docs"
        ));
    }

    #[test]
    fn stellar_docs_url_http_rejected() {
        assert!(!is_stellar_docs_url(
            "http://developers.stellar.org/docs"
        ));
    }

    #[test]
    fn stellar_github_url_valid() {
        assert!(is_stellar_github_url(
            "https://github.com/stellar/stellar-cli"
        ));
    }

    #[test]
    fn tollcraft_docs_url_valid() {
        assert!(is_tollcraft_docs_url(
            "https://tollcraft.gitbook.io/docs/budget-assert"
        ));
    }

    #[test]
    fn tollcraft_docs_url_wrong_host() {
        assert!(!is_tollcraft_docs_url(
            "https://other.gitbook.io/docs"
        ));
    }

    #[test]
    fn invalid_scheme_rejected() {
        assert!(!is_github_repo_url("ftp://github.com/Tollcraft/repo"));
    }

    #[test]
    fn empty_string_rejected() {
        assert!(!is_github_repo_url(""));
        assert!(!is_stellar_docs_url(""));
    }

    #[test]
    fn url_with_trailing_slash() {
        assert!(is_github_repo_url(
            "https://github.com/Tollcraft/soroban-budget-assert/"
        ));
    }
}
