use crate::fixture::FixtureFile;
use crate::transport::{deploy_key, invoke_key, network_id, parse_key, simulate_key, Transport};
use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

/// Serves recorded responses back from a [`FixtureFile`], letting tests run
/// the full report pipeline without a network.
///
/// [`ReplayTransport`] is the read-side counterpart to the record-side
/// transport used when capturing fixtures.  Every method looks up its
/// response in the [`entries`](ReplayTransport::entries) map by the same key
/// scheme ([`deploy_key`], [`invoke_key`], [`simulate_key`]) that was used
/// when the fixture was written, so key construction must stay in sync with
/// the recorder.
///
/// Keys carry the network and source account, so a response is only served
/// for the context it was recorded in. A lookup for any other network or
/// source account is rejected, and the error names the context the fixture
/// was recorded in.
#[cfg_attr(not(test), allow(dead_code))]
pub struct ReplayTransport {
    /// All pre-recorded responses, keyed by the same strings produced by
    /// [`deploy_key`], [`invoke_key`], and [`simulate_key`].
    entries: HashMap<String, serde_json::Value>,
}

impl ReplayTransport {
    /// Construct a [`ReplayTransport`] from a loaded [`FixtureFile`].
    ///
    /// The fixture's `entries` map is moved in directly — no copying or
    /// re-parsing is required.
    ///
    /// `#[cfg_attr(not(test), allow(dead_code))]` mirrors the struct
    /// attribute: both the type and its constructor are only exercised by
    /// tests today, so the compiler would otherwise warn about unused items
    /// in non-test builds.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new(fixture: FixtureFile) -> Self {
        ReplayTransport {
            entries: fixture.entries,
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn get_string_entry(&self, key: &str) -> Result<String> {
        self.entries
            .get(key)
            .and_then(|v| v.as_str().map(String::from))
            .with_context(|| format!("Fixture not found for {key}"))
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn get_value_entry(&self, key: &str) -> Result<Value> {
        self.entries
            .get(key)
            .cloned()
            .with_context(|| format!("Fixture not found for {key}"))
    }

    /// Look up `key`, which was built for `verb` on `target` (the package, or
    /// `package:function`) in the given network and source context.
    fn lookup(
        &self,
        key: &str,
        verb: &str,
        target: &str,
        network: &str,
        source: &str,
    ) -> Result<&Value> {
        match self.entries.get(key) {
            Some(value) => Ok(value),
            None => Err(self.missing_entry(key, verb, target, network, source)),
        }
    }

    /// Explain a lookup miss. A response recorded for the same call in a
    /// different network or source context is refused rather than served,
    /// and the error names the context it was recorded in.
    fn missing_entry(
        &self,
        key: &str,
        verb: &str,
        target: &str,
        network: &str,
        source: &str,
    ) -> anyhow::Error {
        let network = network_id(network);
        let source = source.trim();

        let mut recorded: Vec<(&str, &str)> = self
            .entries
            .keys()
            .filter_map(|k| parse_key(k.as_str()))
            .filter(|parts| parts.verb == verb && parts.target == target)
            .map(|parts| (parts.network, parts.source))
            .collect();
        recorded.sort_unstable();
        recorded.dedup();

        if !recorded.is_empty() {
            let contexts = recorded
                .iter()
                .map(|(n, s)| format!("network `{n}` with source `{s}`"))
                .collect::<Vec<_>>()
                .join(", ");
            return anyhow::anyhow!(
                "Fixture not found for {key}: `{verb}` of `{target}` was recorded for {contexts}, \
                 not network `{network}` with source `{source}`. Refusing to replay a fixture \
                 recorded in a different network context; re-record it with --record"
            );
        }

        let legacy_key = format!("{verb}:{target}");
        if self.entries.contains_key(&legacy_key) {
            return anyhow::anyhow!(
                "Fixture not found for {key}: the fixture has `{legacy_key}`, recorded without a \
                 network or source account, so it cannot be checked against network `{network}`. \
                 Re-record it with --record"
            );
        }

        anyhow::anyhow!("Fixture not found for {key}")
    }
}

/// A deploy or invoke response is a plain string (contract id, XDR).
fn string_entry(key: &str, value: &Value) -> Result<String> {
    value
        .as_str()
        .map(String::from)
        .with_context(|| format!("Fixture entry {key} is not a string"))
}

impl Transport for ReplayTransport {
    fn deploy_contract(
        &mut self,
        _wasm_path: &Path,
        source: &str,
        network: &str,
        package_name: &str,
    ) -> Result<String> {
        // The deploy response is a plain contract-id string, so `.as_str()`
        // is the correct extractor.  A missing key means the fixture was
        // recorded without this package, or in another network context.
        let key = deploy_key(network, source, package_name);
        let value = self.lookup(&key, "deploy", package_name, network, source)?;
        string_entry(&key, value)
    }

    fn build_invoke_xdr(
        &mut self,
        _contract_id: &str,
        source: &str,
        network: &str,
        function: &str,
        _func_args: &[String],
        package: &str,
    ) -> Result<String> {
        // The invoke response is an XDR string (base-64 encoded transaction
        // envelope).  The key encodes both the package and the function name
        // so different entry points on the same contract are stored separately.
        let key = invoke_key(network, source, package, function);
        let target = format!("{package}:{function}");
        let value = self.lookup(&key, "invoke", &target, network, source)?;
        string_entry(&key, value)
    }

    fn simulate_transaction(
        &mut self,
        _b64_xdr: &str,
        source: &str,
        network: &str,
        package: &str,
        function: &str,
    ) -> Result<Value> {
        // The simulate response is a full JSON object (the RPC SimulateTransaction
        // result), so `.cloned()` is used rather than `.as_str()` to preserve
        // the entire structure for downstream budget extraction.
        let key = simulate_key(network, source, package, function);
        let target = format!("{package}:{function}");
        self.lookup(&key, "simulate", &target, network, source)
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_replay_transport_helpers() {
        let mut entries = HashMap::new();
        entries.insert(
            "test_key".to_string(),
            Value::String("test_val".to_string()),
        );
        let transport = ReplayTransport { entries };
        let val = transport.get_string_entry("test_key").unwrap();
        assert_eq!(val, "test_val");
    }

    /// A fixture recorded on `testnet` as `alice`.
    fn testnet_alice() -> ReplayTransport {
        ReplayTransport {
            entries: HashMap::from([
                (deploy_key("testnet", "alice", "pkg"), json!("CONTRACT_ID")),
                (invoke_key("testnet", "alice", "pkg", "f"), json!("XDR")),
                (
                    simulate_key("testnet", "alice", "pkg", "f"),
                    json!({ "result": {} }),
                ),
            ]),
        }
    }

    #[test]
    fn replays_in_the_recorded_context() {
        let mut replay = testnet_alice();
        let path = Path::new("c.wasm");
        assert_eq!(
            replay
                .deploy_contract(path, "alice", "testnet", "pkg")
                .unwrap(),
            "CONTRACT_ID"
        );
        assert_eq!(
            replay
                .build_invoke_xdr("C1", "alice", "testnet", "f", &[], "pkg")
                .unwrap(),
            "XDR"
        );
        assert_eq!(
            replay
                .simulate_transaction("XDR", "alice", "testnet", "pkg", "f")
                .unwrap(),
            json!({ "result": {} })
        );
    }

    #[test]
    fn network_aliases_replay_the_same_fixture() {
        let mut replay = testnet_alice();
        assert_eq!(
            replay
                .deploy_contract(Path::new("c.wasm"), "alice", "test", "pkg")
                .unwrap(),
            "CONTRACT_ID"
        );
    }

    #[test]
    fn rejects_a_different_network() {
        let mut replay = testnet_alice();
        let deploy = replay
            .deploy_contract(Path::new("c.wasm"), "alice", "mainnet", "pkg")
            .unwrap_err()
            .to_string();
        assert!(
            deploy.contains("recorded for network `testnet` with source `alice`"),
            "{deploy}"
        );
        assert!(
            deploy.contains("not network `mainnet` with source `alice`"),
            "{deploy}"
        );
        assert!(deploy.contains("Refusing to replay"), "{deploy}");

        let invoke = replay
            .build_invoke_xdr("C1", "alice", "futurenet", "f", &[], "pkg")
            .unwrap_err()
            .to_string();
        assert!(invoke.contains("Refusing to replay"), "{invoke}");

        let simulate = replay
            .simulate_transaction("XDR", "alice", "mainnet", "pkg", "f")
            .unwrap_err()
            .to_string();
        assert!(simulate.contains("Refusing to replay"), "{simulate}");
    }

    #[test]
    fn rejects_a_different_source_account() {
        let mut replay = testnet_alice();
        let deploy = replay
            .deploy_contract(Path::new("c.wasm"), "bob", "testnet", "pkg")
            .unwrap_err()
            .to_string();
        assert!(
            deploy.contains("not network `testnet` with source `bob`"),
            "{deploy}"
        );

        let simulate = replay
            .simulate_transaction("XDR", "bob", "testnet", "pkg", "f")
            .unwrap_err()
            .to_string();
        assert!(simulate.contains("Refusing to replay"), "{simulate}");
    }

    #[test]
    fn a_call_never_recorded_is_simply_not_found() {
        let mut replay = testnet_alice();
        let err = replay
            .simulate_transaction("XDR", "alice", "testnet", "pkg", "other")
            .unwrap_err()
            .to_string();
        assert_eq!(
            err,
            "Fixture not found for simulate:testnet:alice:pkg:other"
        );
    }

    #[test]
    fn keys_without_network_context_ask_for_a_re_record() {
        let mut replay = ReplayTransport {
            entries: HashMap::from([("deploy:pkg".to_string(), json!("CONTRACT_ID"))]),
        };
        let err = replay
            .deploy_contract(Path::new("c.wasm"), "alice", "testnet", "pkg")
            .unwrap_err()
            .to_string();
        assert!(err.contains("recorded without a network"), "{err}");
        assert!(err.contains("Re-record it with --record"), "{err}");
    }
}
