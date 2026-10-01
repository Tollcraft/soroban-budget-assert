//! URL classification helpers for the hosts this project links to.
//!
//! Each predicate answers one narrow question — for example, "is this string
//! an `https` URL whose authority is exactly `github.com` and whose text names
//! the `Tollcraft` organisation?" — and is deliberately strict. The module
//! exists so link validation never has to trust a substring or suffix match:
//! a lookalike host such as `github.com.evil.example`, a subdomain such as
//! `gist.github.com`, an explicit port, or a non-`https` scheme must all be
//! rejected.
//!
//! All four predicates are built on `check_url_scheme_host`, a small
//! hand-rolled parse rather than a pulled-in URL crate. It matches the scheme
//! and authority literally and case-sensitively; see its documentation for the
//! exact boundary rules. The functions are presently exercised only by the
//! unit tests below, hence the module-wide `#![allow(dead_code)]`.
#![allow(dead_code)]

/// Checks that `url` starts with `expected_scheme` followed by `://`, and that
/// the authority (everything up to the first `/`, or the end of the string)
/// equals `expected_host` exactly.
///
/// The comparison is intentionally literal, which is what makes the public
/// predicates safe against spoofed hosts:
///
/// * Surrounding whitespace on `url` is trimmed before parsing.
/// * The scheme and host are matched case-sensitively, so `HTTPS://github.com`
///   and `https://GitHub.com` do not match `https` / `github.com`.
/// * The authority is read verbatim up to the first `/`. A subdomain
///   (`gist.github.com`), a lookalike suffix (`github.com.evil.example`), a
///   trailing dot (`github.com.`), an explicit port (`github.com:443`),
///   userinfo (`user@github.com`), or a query/fragment with no preceding `/`
///   (`github.com?x=1`) therefore all fail to match.
/// * Only the first `/` terminates the authority; everything after it is path,
///   query or fragment and is ignored by the host comparison.
///
/// Returns `false` for any input that does not carry a well-formed
/// `scheme://host` prefix.
fn check_url_scheme_host(url: &str, expected_scheme: &str, expected_host: &str) -> bool {
    // Trim first so a URL pasted with stray leading/trailing whitespace still
    // parses. Interior whitespace is left alone and fails the match below.
    let trimmed = url.trim();

    // The scheme is a literal, case-sensitive prefix. `starts_with` alone is
    // not sufficient: `httpsfoo://` also starts with `https`, which is exactly
    // why the `://` guard below is required.
    if !trimmed.starts_with(expected_scheme) {
        return false;
    }

    // Everything after the scheme must be the `://` authority marker. This
    // rejects both `https:/github.com` and bare schemes such as `https`.
    let after_scheme = &trimmed[expected_scheme.len()..];
    if !after_scheme.starts_with("://") {
        return false;
    }

    // The authority runs up to the first `/` (or the end of the string). There
    // is no port/userinfo/query parsing: the whole span is compared verbatim,
    // so any decoration fails the equality check.
    let after_protocol = &after_scheme[3..];
    let host_end = after_protocol.find('/').unwrap_or(after_protocol.len());
    let host = &after_protocol[..host_end];

    host == expected_host
}

/// Returns `true` when `url` is an `https://github.com` URL whose text contains
/// the `/Tollcraft/` organisation segment.
///
/// The host must match exactly, so `gitlab.com`, `gist.github.com`,
/// `github.com.evil.example` and non-`https` schemes are rejected. The
/// organisation check, by contrast, is a plain substring search over the whole
/// URL rather than a parsed path segment, so `/Tollcraft/` appearing in a query
/// or fragment also counts — see `org_needle_is_matched_anywhere_in_the_url`
/// for the pinned behaviour.
///
/// # Examples
///
/// ```text
/// https://github.com/Tollcraft/soroban-budget-assert  -> true
/// https://github.com/other-org/repo                   -> false
/// http://github.com/Tollcraft/repo                    -> false
/// ```
pub fn is_github_repo_url(url: &str) -> bool {
    // `check_url_scheme_host` trims internally, while the org needle is
    // searched in the original string. Surrounding whitespace cannot change a
    // substring match, so the two halves of the check stay consistent.
    check_url_scheme_host(url, "https", "github.com") && url.contains("/Tollcraft/")
}

/// Returns `true` when `url` is an `https` URL on exactly
/// `developers.stellar.org`.
///
/// The parent domain (`stellar.org`), subdomains
/// (`docs.developers.stellar.org`) and lookalike suffixes are all rejected, and
/// a trailing-dot FQDN (`developers.stellar.org.`) does not match. Path, query
/// and fragment are ignored once the authority matches.
///
/// # Examples
///
/// ```text
/// https://developers.stellar.org/docs/reference/rpc  -> true
/// https://stellar.org/docs                           -> false
/// http://developers.stellar.org/docs                 -> false
/// ```
pub fn is_stellar_docs_url(url: &str) -> bool {
    check_url_scheme_host(url, "https", "developers.stellar.org")
}

/// Returns `true` when `url` is an `https://github.com` URL whose text contains
/// the `/stellar/` organisation segment.
///
/// Mirrors `is_github_repo_url` with the Stellar organisation as the needle,
/// including the substring (rather than path-segment) matching of that needle.
///
/// # Examples
///
/// ```text
/// https://github.com/stellar/stellar-cli  -> true
/// https://github.com/Tollcraft/repo       -> false
/// https://gitlab.com/stellar/stellar-cli  -> false
/// ```
pub fn is_stellar_github_url(url: &str) -> bool {
    check_url_scheme_host(url, "https", "github.com") && url.contains("/stellar/")
}

/// Returns `true` when `url` is an `https` URL on exactly
/// `tollcraft.gitbook.io`.
///
/// Other GitBook spaces (`other.gitbook.io`), the bare GitBook host
/// (`gitbook.io`), subdomains and lookalike suffixes are rejected.
///
/// # Examples
///
/// ```text
/// https://tollcraft.gitbook.io/docs/budget-assert  -> true
/// https://other.gitbook.io/docs                    -> false
/// https://tollcraft.gitbook.io.evil.example/docs   -> false
/// ```
pub fn is_tollcraft_docs_url(url: &str) -> bool {
    check_url_scheme_host(url, "https", "tollcraft.gitbook.io")
}

#[cfg(test)]
mod tests {
    use super::*;

    // =================================================================
    // Happy paths (one per public predicate)
    // =================================================================

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
        assert!(!is_github_repo_url("https://github.com/other-org/repo"));
    }

    #[test]
    fn stellar_docs_url_valid() {
        assert!(is_stellar_docs_url(
            "https://developers.stellar.org/docs/learn/fundamentals/fees"
        ));
    }

    #[test]
    fn stellar_docs_url_wrong_domain() {
        assert!(!is_stellar_docs_url("https://stellar.org/docs"));
    }

    #[test]
    fn stellar_docs_url_http_rejected() {
        assert!(!is_stellar_docs_url("http://developers.stellar.org/docs"));
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
        assert!(!is_tollcraft_docs_url("https://other.gitbook.io/docs"));
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

    // =================================================================
    // `check_url_scheme_host` — the shared scheme/host parser
    // =================================================================

    mod scheme_host_parser {
        use super::*;

        #[test]
        fn accepts_exact_scheme_and_host() {
            assert!(check_url_scheme_host(
                "https://github.com/Tollcraft/x",
                "https",
                "github.com"
            ));
        }

        #[test]
        fn accepts_bare_host_with_no_path() {
            assert!(check_url_scheme_host(
                "https://github.com",
                "https",
                "github.com"
            ));
        }

        #[test]
        fn accepts_host_followed_by_slash_only() {
            assert!(check_url_scheme_host(
                "https://github.com/",
                "https",
                "github.com"
            ));
        }

        #[test]
        fn host_is_read_only_up_to_the_first_slash() {
            // Everything after the first `/` is path and must not affect the
            // host match.
            assert!(check_url_scheme_host(
                "https://github.com/a/b/c?x=1#frag",
                "https",
                "github.com"
            ));
        }

        #[test]
        fn scheme_prefix_must_be_followed_by_double_slash() {
            assert!(!check_url_scheme_host(
                "https:/github.com",
                "https",
                "github.com"
            ));
        }

        #[test]
        fn scheme_prefix_with_no_remainder_is_rejected() {
            assert!(!check_url_scheme_host("https", "https", "github.com"));
        }

        #[test]
        fn extra_letters_after_scheme_are_rejected() {
            // `starts_with("https")` passes, but `httpsfoo://` is not a valid
            // `https://` authority, so the `://` guard must reject it.
            assert!(!check_url_scheme_host(
                "httpsfoo://github.com",
                "https",
                "github.com"
            ));
        }

        #[test]
        fn scheme_match_is_case_sensitive() {
            assert!(!check_url_scheme_host(
                "HTTPS://github.com",
                "https",
                "github.com"
            ));
            assert!(!check_url_scheme_host(
                "Http://github.com",
                "http",
                "github.com"
            ));
        }

        #[test]
        fn host_match_is_case_sensitive() {
            assert!(!check_url_scheme_host(
                "https://GitHub.com/x",
                "https",
                "github.com"
            ));
        }

        #[test]
        fn surrounding_whitespace_is_trimmed() {
            assert!(check_url_scheme_host(
                "  https://github.com/x  ",
                "https",
                "github.com"
            ));
            assert!(check_url_scheme_host(
                "\thttps://github.com/x\n",
                "https",
                "github.com"
            ));
        }

        #[test]
        fn inner_whitespace_is_not_trimmed() {
            assert!(!check_url_scheme_host(
                "https:// github.com/x",
                "https",
                "github.com"
            ));
        }

        #[test]
        fn subdomains_do_not_match_the_bare_host() {
            assert!(!check_url_scheme_host(
                "https://gist.github.com/x",
                "https",
                "github.com"
            ));
            assert!(!check_url_scheme_host(
                "https://docs.developers.stellar.org/x",
                "https",
                "developers.stellar.org"
            ));
        }

        #[test]
        fn host_suffix_attacks_are_rejected() {
            // A lookalike host that merely *starts with* the expected host must
            // not pass: the comparison is on the whole authority.
            assert!(!check_url_scheme_host(
                "https://github.com.evil.example/x",
                "https",
                "github.com"
            ));
            assert!(!check_url_scheme_host(
                "https://tollcraft.gitbook.io.evil.example/x",
                "https",
                "tollcraft.gitbook.io"
            ));
        }

        #[test]
        fn trailing_dot_fqdn_is_rejected() {
            // `github.com.` is a valid FQDN spelling but not the literal host
            // this helper matches; document the exact-match behaviour.
            assert!(!check_url_scheme_host(
                "https://github.com./x",
                "https",
                "github.com"
            ));
        }

        #[test]
        fn explicit_port_is_rejected() {
            // The authority is compared verbatim, so an explicit (even
            // default) port does not match the bare host.
            assert!(!check_url_scheme_host(
                "https://github.com:443/x",
                "https",
                "github.com"
            ));
        }

        #[test]
        fn userinfo_prefix_is_rejected() {
            assert!(!check_url_scheme_host(
                "https://user@github.com/x",
                "https",
                "github.com"
            ));
        }

        #[test]
        fn query_or_fragment_directly_after_host_is_rejected() {
            // Without a `/` separator the whole `host?query` span is treated as
            // the authority, so these do not match the bare host.
            assert!(!check_url_scheme_host(
                "https://github.com?x=1",
                "https",
                "github.com"
            ));
            assert!(!check_url_scheme_host(
                "https://github.com#frag",
                "https",
                "github.com"
            ));
        }

        #[test]
        fn empty_authority_matches_an_empty_expected_host() {
            assert!(check_url_scheme_host("https:///x", "https", ""));
        }

        #[test]
        fn empty_authority_does_not_match_a_real_host() {
            assert!(!check_url_scheme_host("https:///x", "https", "github.com"));
        }

        #[test]
        fn empty_expected_scheme_still_requires_the_authority_marker() {
            // With an empty scheme the `://` marker is all that is required
            // before the authority.
            assert!(check_url_scheme_host("://github.com", "", "github.com"));
            assert!(!check_url_scheme_host("github.com", "", "github.com"));
        }

        #[test]
        fn non_ascii_path_does_not_affect_host_match() {
            assert!(check_url_scheme_host(
                "https://github.com/日本語/パス",
                "https",
                "github.com"
            ));
        }

        #[test]
        fn non_ascii_authority_does_not_match_ascii_expected_host() {
            assert!(!check_url_scheme_host(
                "https://gíthub.com/x",
                "https",
                "github.com"
            ));
        }

        #[test]
        fn any_scheme_can_be_requested() {
            assert!(check_url_scheme_host(
                "ftp://files.example/x",
                "ftp",
                "files.example"
            ));
            assert!(check_url_scheme_host(
                "http://localhost:8000/soroban/rpc",
                "http",
                "localhost:8000"
            ));
        }
    }

    // =================================================================
    // `is_github_repo_url`
    // =================================================================

    mod github_repo_url {
        use super::*;

        #[test]
        fn requires_the_slash_wrapped_org_segment() {
            // A bare `github.com/Tollcraft` (no trailing slash) does not
            // contain the `/Tollcraft/` needle.
            assert!(!is_github_repo_url("https://github.com/Tollcraft"));
        }

        #[test]
        fn org_match_is_case_sensitive() {
            assert!(!is_github_repo_url(
                "https://github.com/tollcraft/soroban-budget-assert"
            ));
        }

        #[test]
        fn accepts_a_nested_path_under_the_org() {
            assert!(is_github_repo_url(
                "https://github.com/Tollcraft/soroban-budget-assert/tree/main/src"
            ));
        }

        #[test]
        fn rejects_other_orgs_on_github() {
            assert!(!is_github_repo_url(
                "https://github.com/stellar/stellar-cli"
            ));
            assert!(!is_github_repo_url("https://github.com/other-org/repo"));
        }

        #[test]
        fn rejects_the_org_on_a_lookalike_host() {
            assert!(!is_github_repo_url(
                "https://github.com.evil.example/Tollcraft/repo"
            ));
            assert!(!is_github_repo_url(
                "https://gist.github.com/Tollcraft/1234"
            ));
        }

        #[test]
        fn rejects_http_and_other_schemes() {
            assert!(!is_github_repo_url("http://github.com/Tollcraft/repo"));
            assert!(!is_github_repo_url("ssh://github.com/Tollcraft/repo"));
            assert!(!is_github_repo_url("git@github.com:Tollcraft/repo.git"));
        }

        #[test]
        fn tolerates_surrounding_whitespace() {
            assert!(is_github_repo_url(
                "  https://github.com/Tollcraft/soroban-budget-assert  "
            ));
        }

        #[test]
        fn org_needle_is_matched_anywhere_in_the_url() {
            // Characterisation test: the org check is a substring search over
            // the whole URL, not a path-segment check, so a `/Tollcraft/`
            // needle outside the path still satisfies it. Pinning this down
            // keeps a future tightening of the check visible.
            assert!(is_github_repo_url(
                "https://github.com/other-org/repo?ref=/Tollcraft/x"
            ));
        }
    }

    // =================================================================
    // `is_stellar_docs_url`
    // =================================================================

    mod stellar_docs_url {
        use super::*;

        #[test]
        fn accepts_bare_host() {
            assert!(is_stellar_docs_url("https://developers.stellar.org"));
        }

        #[test]
        fn accepts_any_documentation_path() {
            assert!(is_stellar_docs_url(
                "https://developers.stellar.org/docs/learn/fundamentals/fees"
            ));
            assert!(is_stellar_docs_url(
                "https://developers.stellar.org/docs/reference/rpc"
            ));
        }

        #[test]
        fn rejects_the_parent_domain() {
            assert!(!is_stellar_docs_url("https://stellar.org/docs"));
            assert!(!is_stellar_docs_url("https://www.stellar.org/docs"));
        }

        #[test]
        fn rejects_subdomains_of_the_docs_host() {
            assert!(!is_stellar_docs_url(
                "https://docs.developers.stellar.org/docs"
            ));
        }

        #[test]
        fn rejects_http() {
            assert!(!is_stellar_docs_url("http://developers.stellar.org/docs"));
        }

        #[test]
        fn rejects_a_trailing_dot_host() {
            assert!(!is_stellar_docs_url("https://developers.stellar.org./docs"));
        }

        #[test]
        fn rejects_empty_and_whitespace_only_input() {
            assert!(!is_stellar_docs_url(""));
            assert!(!is_stellar_docs_url("   "));
        }

        #[test]
        fn tolerates_surrounding_whitespace() {
            assert!(is_stellar_docs_url(
                "\n  https://developers.stellar.org/docs  \t"
            ));
        }
    }

    // =================================================================
    // `is_stellar_github_url`
    // =================================================================

    mod stellar_github_url {
        use super::*;

        #[test]
        fn accepts_repositories_under_the_stellar_org() {
            assert!(is_stellar_github_url(
                "https://github.com/stellar/stellar-cli"
            ));
            assert!(is_stellar_github_url(
                "https://github.com/stellar/rs-soroban-env/tree/main"
            ));
        }

        #[test]
        fn requires_the_slash_wrapped_org_segment() {
            assert!(!is_stellar_github_url("https://github.com/stellar"));
        }

        #[test]
        fn org_match_is_case_sensitive() {
            assert!(!is_stellar_github_url(
                "https://github.com/Stellar/stellar-cli"
            ));
        }

        #[test]
        fn rejects_other_orgs_on_github() {
            assert!(!is_stellar_github_url("https://github.com/Tollcraft/repo"));
            assert!(!is_stellar_github_url(
                "https://github.com/other/stellar-cli"
            ));
        }

        #[test]
        fn rejects_non_github_hosts() {
            assert!(!is_stellar_github_url(
                "https://gitlab.com/stellar/stellar-cli"
            ));
            assert!(!is_stellar_github_url(
                "https://github.com.evil.example/stellar/stellar-cli"
            ));
        }

        #[test]
        fn rejects_non_https_schemes() {
            assert!(!is_stellar_github_url(
                "http://github.com/stellar/stellar-cli"
            ));
            assert!(!is_stellar_github_url(
                "ssh://github.com/stellar/stellar-cli"
            ));
        }

        #[test]
        fn rejects_empty_input() {
            assert!(!is_stellar_github_url(""));
        }
    }

    // =================================================================
    // `is_tollcraft_docs_url`
    // =================================================================

    mod tollcraft_docs_url {
        use super::*;

        #[test]
        fn accepts_bare_host() {
            assert!(is_tollcraft_docs_url("https://tollcraft.gitbook.io"));
        }

        #[test]
        fn accepts_any_documentation_path() {
            assert!(is_tollcraft_docs_url(
                "https://tollcraft.gitbook.io/docs/budget-assert"
            ));
        }

        #[test]
        fn rejects_other_gitbook_spaces() {
            assert!(!is_tollcraft_docs_url("https://other.gitbook.io/docs"));
            assert!(!is_tollcraft_docs_url("https://gitbook.io/docs"));
        }

        #[test]
        fn rejects_subdomains_and_suffixes() {
            assert!(!is_tollcraft_docs_url(
                "https://docs.tollcraft.gitbook.io/docs"
            ));
            assert!(!is_tollcraft_docs_url(
                "https://tollcraft.gitbook.io.evil.example/docs"
            ));
        }

        #[test]
        fn rejects_http() {
            assert!(!is_tollcraft_docs_url("http://tollcraft.gitbook.io/docs"));
        }

        #[test]
        fn rejects_empty_input() {
            assert!(!is_tollcraft_docs_url(""));
        }

        #[test]
        fn tolerates_surrounding_whitespace() {
            assert!(is_tollcraft_docs_url(
                "  https://tollcraft.gitbook.io/docs/budget-assert\n"
            ));
        }
    }

    // =================================================================
    // Cross-predicate host isolation
    // =================================================================

    mod cross_predicate_isolation {
        use super::*;

        #[test]
        fn each_github_predicate_rejects_the_others_org() {
            // Distinct org needles must not leak between the two GitHub
            // predicates.
            assert!(!is_stellar_github_url("https://github.com/Tollcraft/repo"));
            assert!(!is_github_repo_url("https://github.com/stellar/repo"));
        }

        #[test]
        fn docs_predicates_reject_each_others_hosts() {
            assert!(!is_stellar_docs_url("https://tollcraft.gitbook.io/docs"));
            assert!(!is_tollcraft_docs_url(
                "https://developers.stellar.org/docs"
            ));
        }

        #[test]
        fn github_hosts_are_rejected_by_docs_predicates() {
            assert!(!is_stellar_docs_url(
                "https://github.com/stellar/stellar-cli"
            ));
            assert!(!is_tollcraft_docs_url("https://github.com/Tollcraft/repo"));
        }

        #[test]
        fn a_stellar_github_url_is_not_a_stellar_docs_url() {
            let url = "https://github.com/stellar/stellar-cli";
            assert!(is_stellar_github_url(url));
            assert!(!is_stellar_docs_url(url));
        }

        #[test]
        fn only_the_github_org_predicate_accepts_the_project_repo() {
            let url = "https://github.com/Tollcraft/soroban-budget-assert";
            assert!(is_github_repo_url(url));
            assert!(!is_stellar_github_url(url));
            assert!(!is_stellar_docs_url(url));
            assert!(!is_tollcraft_docs_url(url));
        }

        #[test]
        fn only_the_tollcraft_predicate_accepts_the_gitbook_site() {
            let url = "https://tollcraft.gitbook.io/docs/budget-assert";
            assert!(is_tollcraft_docs_url(url));
            assert!(!is_github_repo_url(url));
            assert!(!is_stellar_github_url(url));
            assert!(!is_stellar_docs_url(url));
        }
    }

    // =================================================================
    // Malformed input sweep
    // =================================================================

    mod malformed_input {
        use super::*;

        /// Inputs that must be rejected by every predicate: none of them has a
        /// well-formed `https://` authority.
        const ALL_REJECTED: &[&str] = &[
            "",
            " ",
            "\n\t",
            "https",
            "https:",
            "https:/",
            "https://",
            "not-a-url",
            "//github.com/Tollcraft/repo",
            "github.com/Tollcraft/repo",
            "https//github.com/Tollcraft/repo",
            "http://github.com/Tollcraft/repo",
            "javascript:alert(1)",
            "data:text/plain,https://github.com/Tollcraft/repo",
        ];

        #[test]
        fn github_repo_rejects_every_malformed_input() {
            for url in ALL_REJECTED {
                assert!(!is_github_repo_url(url), "should reject {url:?}");
            }
        }

        #[test]
        fn stellar_docs_rejects_every_malformed_input() {
            for url in ALL_REJECTED {
                assert!(!is_stellar_docs_url(url), "should reject {url:?}");
            }
        }

        #[test]
        fn stellar_github_rejects_every_malformed_input() {
            for url in ALL_REJECTED {
                assert!(!is_stellar_github_url(url), "should reject {url:?}");
            }
        }

        #[test]
        fn tollcraft_docs_rejects_every_malformed_input() {
            for url in ALL_REJECTED {
                assert!(!is_tollcraft_docs_url(url), "should reject {url:?}");
            }
        }
    }
}
