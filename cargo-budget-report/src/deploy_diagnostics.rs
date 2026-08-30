//! Turn a failed deploy's error text into an actionable message.
//!
//! Deployment funds a source account and pushes a contract through the
//! `stellar` CLI, retried a few times with exponential backoff for
//! plausibly-transient failures (see [`crate::run_with_retry`] and
//! [`crate::is_transient_error`]). When every attempt fails, "deploy failed
//! after N attempts" on its own does not tell the three failure classes
//! apart — rate limiting, a service outage, and an unreachable network each
//! call for a different response, and an unfunded account will not resolve
//! by waiting at all.

/// Which kind of failure a spent deploy retry loop hit, inferred from the
/// last error text the `stellar` CLI / transport produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeployFailureClass {
    /// Friendbot or the RPC is rate-limiting this IP (HTTP 429).
    RateLimited,
    /// The service answered with a server-side error (5xx / "unavailable").
    ServiceUnavailable,
    /// The network could not be reached at all (DNS, connection, timeout).
    NetworkUnreachable,
    /// The source account is missing or has no balance on this network.
    AccountUnfunded,
    /// Nothing in the text matched a known class.
    Unknown,
}

impl DeployFailureClass {
    /// Guidance line(s) for this class, given the `source` identity and
    /// `network` the run was using.
    pub fn guidance(self, source: &str, network: &str) -> String {
        match self {
            DeployFailureClass::RateLimited => {
                "Cause: rate limiting. Friendbot / the RPC is throttling requests from this IP.\n  \
                 What to do: wait ~60 seconds and re-run — the limit is per-IP and lifts on its \
                 own. Running from CI on a shared runner makes this more likely; a short sleep \
                 before the step usually clears it."
                    .to_string()
            }
            DeployFailureClass::ServiceUnavailable => {
                "Cause: the deploy/funding service returned a server error — it is down or \
                 overloaded, not something on your side.\n  \
                 What to do: check https://status.stellar.org, then re-run in a few minutes. \
                 This is shared infrastructure; there is nothing to fix locally."
                    .to_string()
            }
            DeployFailureClass::NetworkUnreachable => {
                "Cause: the network could not be reached (DNS / connection / timeout).\n  \
                 What to do: check your own connectivity, any proxy or VPN, and firewall rules \
                 for outbound HTTPS to Stellar infrastructure, then re-run."
                    .to_string()
            }
            DeployFailureClass::AccountUnfunded => format!(
                "Cause: the source account '{source}' is missing or unfunded on {network}.\n  \
                 What to do: fund it and re-run — `stellar keys fund {source} --network {network}` \
                 (or `stellar keys generate {source} --network {network} --fund` to create it). \
                 This will not resolve by waiting."
            ),
            DeployFailureClass::Unknown => format!(
                "The error text did not match a known failure class. The most common cause is an \
                 unfunded source account: confirm '{source}' exists (`stellar keys ls`) and is \
                 funded on {network}."
            ),
        }
    }
}

/// Classify a deploy failure from the last error string the transport
/// surfaced. Order matters: the account-state checks come first because an
/// unfunded account is the one class that waiting cannot fix.
pub fn classify(last_error: &str) -> DeployFailureClass {
    let lowered = last_error.to_ascii_lowercase();
    let has = |needle: &str| lowered.contains(needle);

    if has("txinsufficientbalance")
        || has("insufficient balance")
        || has("underfunded")
        || has("account not found")
        || has("accountnotfound")
        || has("account does not exist")
        || has("could not find account")
        || has("not funded")
        || has("account requires a minimum balance")
    {
        return DeployFailureClass::AccountUnfunded;
    }

    if has("rate limit")
        || has("rate-limit")
        || has("ratelimit")
        || has("429")
        || has("too many requests")
    {
        return DeployFailureClass::RateLimited;
    }

    if has("dns")
        || has("could not resolve")
        || has("name resolution")
        || has("connection refused")
        || has("connection reset")
        || has("reset by peer")
        || has("broken pipe")
        || has("timed out")
        || has("timeout")
        || has("network is unreachable")
        || has("failed to connect")
        || has("no route to host")
    {
        return DeployFailureClass::NetworkUnreachable;
    }

    if has("503")
        || has("502")
        || has("504")
        || has("500")
        || has("unavailable")
        || has("bad gateway")
        || has("gateway timeout")
        || has("temporarily")
        || has("try again")
        || has("internal server error")
    {
        return DeployFailureClass::ServiceUnavailable;
    }

    DeployFailureClass::Unknown
}

/// A one-line summary of an error, for the per-attempt retry progress line.
/// Collapses whitespace and truncates so a multi-line CLI error does not
/// scroll the terminal on every backoff.
pub fn summarize(error: &str) -> String {
    let collapsed = error.split_whitespace().collect::<Vec<_>>().join(" ");
    const MAX: usize = 140;
    if collapsed.chars().count() > MAX {
        let mut s: String = collapsed.chars().take(MAX).collect();
        s.push('…');
        s
    } else {
        collapsed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limit_text_classifies_as_rate_limited() {
        for text in [
            "Error: friendbot rate-limited (try again later)",
            "HTTP 429 Too Many Requests",
            "rate limit exceeded",
        ] {
            assert_eq!(classify(text), DeployFailureClass::RateLimited, "{text}");
        }
    }

    #[test]
    fn connection_text_classifies_as_network_unreachable() {
        for text in [
            "error sending request: connection reset by peer",
            "curl: (6) Could not resolve host: soroban-testnet.stellar.org",
            "operation timed out after 30000 milliseconds",
        ] {
            assert_eq!(
                classify(text),
                DeployFailureClass::NetworkUnreachable,
                "{text}"
            );
        }
    }

    #[test]
    fn server_error_text_classifies_as_service_unavailable() {
        for text in [
            "received 503 Service Unavailable from friendbot",
            "502 Bad Gateway",
            "the service is temporarily unavailable",
        ] {
            assert_eq!(
                classify(text),
                DeployFailureClass::ServiceUnavailable,
                "{text}"
            );
        }
    }

    #[test]
    fn unfunded_account_text_takes_priority() {
        // Contains "try again" (server-ish) but the account signal wins.
        let text = "txInsufficientBalance: source account underfunded, try again";
        assert_eq!(classify(text), DeployFailureClass::AccountUnfunded);
    }

    #[test]
    fn unknown_text_is_unknown() {
        assert_eq!(
            classify("some entirely novel failure"),
            DeployFailureClass::Unknown
        );
    }

    #[test]
    fn rate_limit_guidance_says_how_long_to_wait() {
        let g = DeployFailureClass::RateLimited.guidance("alice", "testnet");
        assert!(
            g.contains("60 seconds"),
            "rate-limit guidance gives a wait: {g}"
        );
    }

    #[test]
    fn unfunded_guidance_names_the_identity_and_the_fix() {
        let g = DeployFailureClass::AccountUnfunded.guidance("alice", "testnet");
        assert!(g.contains("alice"));
        assert!(g.contains("stellar keys fund alice --network testnet"));
        assert!(g.contains("not resolve by waiting"));
    }

    #[test]
    fn summarize_collapses_and_truncates() {
        let multi = "line one\n   line two\n\tline three";
        assert_eq!(summarize(multi), "line one line two line three");

        let long = "x ".repeat(200);
        let s = summarize(&long);
        assert!(s.chars().count() <= 141, "truncated: {}", s.chars().count());
        assert!(s.ends_with('…'));
    }
}
