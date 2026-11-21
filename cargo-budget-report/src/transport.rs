//! Network transport boundary for a measurement run.
//!
//! The module is deliberately split so that the *policy* of a run (what to
//! deploy, which function to call, how to report) is separable from the
//! *I/O* (spawning the `stellar` CLI, POSTing to the RPC endpoint). The
//! [`Transport`] trait is that seam: the live implementation talks to the
//! network, while `fixture` (record/replay) implements the same trait to
//! serve recorded responses with no network access at all.
//!
//! Key construction — the part record and replay must agree on exactly — is
//! factored out of the trait into [`fixture_key`] and its three named
//! wrappers below, each independently unit-tested.
//!
//! Every key carries the network and the source account the response was
//! recorded against, so a fixture recorded on `testnet` as `alice` is never
//! served for `mainnet` or for another source account: the lookup misses and
//! `ReplayTransport` reports which context the fixture was recorded in.

use anyhow::Result;
use serde_json::Value;
use std::path::Path;

pub trait Transport {
    fn deploy_contract(
        &mut self,
        wasm_path: &Path,
        source: &str,
        network: &str,
        package_name: &str,
    ) -> Result<String>;

    fn build_invoke_xdr(
        &mut self,
        contract_id: &str,
        source: &str,
        network: &str,
        function: &str,
        func_args: &[String],
        package: &str,
    ) -> Result<String>;

    /// `source` and `network` are the context the XDR was built for. The live
    /// transport does not need them, but record and replay key the response
    /// by them so a simulation is never replayed for a different network or
    /// source account.
    fn simulate_transaction(
        &mut self,
        b64_xdr: &str,
        source: &str,
        network: &str,
        package: &str,
        function: &str,
    ) -> Result<Value>;
}

/// Join `parts` with `:` into a single, exactly-sized `String`.
///
/// Fixture keys are built on every recorded or replayed call; sizing the
/// buffer up front avoids the incremental regrowth `format!` does.
fn fixture_key(parts: &[&str]) -> String {
    let len = parts.iter().map(|p| p.len()).sum::<usize>() + parts.len().saturating_sub(1);
    let mut key = String::with_capacity(len);
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            key.push(':');
        }
        key.push_str(part);
    }
    key
}

/// Canonical network identifier stored in fixture keys.
///
/// The built-in aliases accepted by `live::resolve_rpc_url` collapse to one
/// name each, so a fixture recorded with `--network test` replays for
/// `--network testnet`. Any other name is a custom network and is kept as
/// given, trimmed and lower-cased.
pub fn network_id(network: &str) -> String {
    let network = network.trim().to_ascii_lowercase();
    let canonical = match network.as_str() {
        "testnet" | "test" => "testnet",
        "futurenet" | "future" => "futurenet",
        "mainnet" | "pubnet" | "publicnet" | "public" => "mainnet",
        "local" | "localnet" | "standalone" => "local",
        _ => return network,
    };
    canonical.to_string()
}

/// Fixture key for a [`Transport::deploy_contract`] response.
pub fn deploy_key(network: &str, source: &str, package: &str) -> String {
    let network = network_id(network);
    fixture_key(&["deploy", &network, source.trim(), package])
}

/// Fixture key for a [`Transport::build_invoke_xdr`] response.
pub fn invoke_key(network: &str, source: &str, package: &str, function: &str) -> String {
    let network = network_id(network);
    fixture_key(&["invoke", &network, source.trim(), package, function])
}

/// Fixture key for a [`Transport::simulate_transaction`] response.
pub fn simulate_key(network: &str, source: &str, package: &str, function: &str) -> String {
    let network = network_id(network);
    fixture_key(&["simulate", &network, source.trim(), package, function])
}

/// A fixture key split back into its parts, for replay diagnostics.
///
/// `target` is everything after the source account: the package for a deploy
/// key, `package:function` for an invoke or simulate key.
#[derive(Debug, PartialEq, Eq)]
pub struct KeyParts<'a> {
    pub verb: &'a str,
    pub network: &'a str,
    pub source: &'a str,
    pub target: &'a str,
}

/// Split a fixture key into [`KeyParts`].
///
/// Returns `None` for a key without a network and source, such as one written
/// before keys carried them (`deploy:<package>`). Like [`fixture_key`], this is
/// only exact when the network and source contain no `:`.
pub fn parse_key(key: &str) -> Option<KeyParts<'_>> {
    let mut parts = key.splitn(4, ':');
    Some(KeyParts {
        verb: parts.next()?,
        network: parts.next()?,
        source: parts.next()?,
        target: parts.next()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_match_the_fixture_format() {
        assert_eq!(
            deploy_key("testnet", "alice", "pkg"),
            "deploy:testnet:alice:pkg"
        );
        assert_eq!(
            invoke_key("testnet", "alice", "pkg", "do_work"),
            "invoke:testnet:alice:pkg:do_work"
        );
        assert_eq!(
            simulate_key("testnet", "alice", "pkg", "do_work"),
            "simulate:testnet:alice:pkg:do_work"
        );
    }

    #[test]
    fn keys_with_empty_parts_keep_their_separators() {
        assert_eq!(deploy_key("", "", ""), "deploy:::");
        assert_eq!(invoke_key("", "", "", ""), "invoke::::");
        assert_eq!(simulate_key("local", "", "pkg", ""), "simulate:local::pkg:");
    }

    #[test]
    fn keys_are_allocated_at_exact_capacity() {
        for key in [
            deploy_key("testnet", "alice", "pkg"),
            invoke_key("Custom Net", "bob", "päckäge", "fünctiön"),
            simulate_key("", "", "", ""),
        ] {
            assert_eq!(key.capacity(), key.len(), "{key}");
        }
    }

    #[test]
    fn keys_differ_by_network() {
        assert_ne!(
            deploy_key("testnet", "alice", "pkg"),
            deploy_key("mainnet", "alice", "pkg")
        );
        assert_ne!(
            invoke_key("testnet", "alice", "pkg", "f"),
            invoke_key("futurenet", "alice", "pkg", "f")
        );
        assert_ne!(
            simulate_key("testnet", "alice", "pkg", "f"),
            simulate_key("local", "alice", "pkg", "f")
        );
    }

    #[test]
    fn keys_differ_by_source_account() {
        assert_ne!(
            deploy_key("testnet", "alice", "pkg"),
            deploy_key("testnet", "bob", "pkg")
        );
        assert_ne!(
            invoke_key("testnet", "alice", "pkg", "f"),
            invoke_key("testnet", "bob", "pkg", "f")
        );
        assert_ne!(
            simulate_key("testnet", "alice", "pkg", "f"),
            simulate_key("testnet", "bob", "pkg", "f")
        );
    }

    #[test]
    fn network_aliases_share_one_key() {
        for alias in ["testnet", "test", " TestNet "] {
            assert_eq!(network_id(alias), "testnet", "{alias}");
        }
        for alias in ["mainnet", "pubnet", "publicnet", "public"] {
            assert_eq!(network_id(alias), "mainnet", "{alias}");
        }
        for alias in ["futurenet", "future"] {
            assert_eq!(network_id(alias), "futurenet", "{alias}");
        }
        for alias in ["local", "localnet", "standalone"] {
            assert_eq!(network_id(alias), "local", "{alias}");
        }
        assert_eq!(
            deploy_key("test", "alice", "pkg"),
            deploy_key("testnet", "alice", "pkg")
        );
    }

    #[test]
    fn custom_network_names_are_kept() {
        assert_eq!(network_id("My-Private-Net"), "my-private-net");
        assert_ne!(
            deploy_key("private-a", "alice", "pkg"),
            deploy_key("private-b", "alice", "pkg")
        );
    }

    #[test]
    fn parse_key_splits_new_keys() {
        assert_eq!(
            parse_key("deploy:testnet:alice:pkg"),
            Some(KeyParts {
                verb: "deploy",
                network: "testnet",
                source: "alice",
                target: "pkg",
            })
        );
        assert_eq!(
            parse_key(&simulate_key("local", "bob", "pkg", "do_work")),
            Some(KeyParts {
                verb: "simulate",
                network: "local",
                source: "bob",
                target: "pkg:do_work",
            })
        );
    }

    #[test]
    fn parse_key_rejects_keys_without_network_context() {
        assert_eq!(parse_key("deploy:pkg"), None);
        assert_eq!(parse_key("invoke:pkg:do_work"), None);
        assert_eq!(parse_key(""), None);
    }

    #[test]
    fn fixture_key_handles_zero_and_one_part() {
        assert_eq!(fixture_key(&[]), "");
        assert_eq!(fixture_key(&["only"]), "only");
    }

    #[test]
    fn fixture_key_handles_a_single_empty_part() {
        assert_eq!(fixture_key(&[""]), "");
        assert_eq!(fixture_key(&["", ""]), ":");
    }

    #[test]
    fn fixture_key_joins_many_parts_without_a_trailing_separator() {
        assert_eq!(fixture_key(&["a", "b", "c", "d", "e"]), "a:b:c:d:e");
    }

    #[test]
    fn fixture_key_many_parts_are_allocated_at_exact_capacity() {
        let parts = ["alpha", "beta", "gamma", "delta", "epsilon"];
        let key = fixture_key(&parts);
        assert_eq!(key.capacity(), key.len());
    }

    #[test]
    fn fixture_key_separator_is_ambiguous_when_a_part_contains_it() {
        // Characterisation test: parts are joined with `:` and not escaped, so
        // different part lists can produce the same key. Fixture keys are
        // therefore only unambiguous for part values that cannot contain `:`
        // (network aliases, source accounts, package and function names).
        // Pinning this down keeps any future change to the key format visible.
        assert_eq!(
            fixture_key(&["a", "b:c"]),
            fixture_key(&["a:b", "c"]),
            "unescaped `:` in a part collides with the separator"
        );
    }

    #[test]
    fn deploy_and_simulate_keys_never_collide_for_the_same_pair() {
        // The verb prefix is what keeps the three response kinds apart for one
        // (package, function) pair.
        let package = "my-contract";
        let function = "do_work";
        let deploy = deploy_key("testnet", "alice", package);
        let invoke = invoke_key("testnet", "alice", package, function);
        let simulate = simulate_key("testnet", "alice", package, function);
        assert_ne!(deploy, invoke);
        assert_ne!(invoke, simulate);
        assert_ne!(deploy, simulate);
    }
}
