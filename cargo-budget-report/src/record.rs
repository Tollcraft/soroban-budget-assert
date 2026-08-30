use crate::fixture::FixtureFile;
use crate::transport::Transport;
use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

/// Wraps any [`Transport`] and captures every response so the run can be
/// turned into a replayable fixture with [`RecordingTransport::into_fixture`].
///
/// Wired for tests today; a CLI flag to record a real run is the follow-up
/// that makes this the production record path.
#[cfg_attr(not(test), allow(dead_code))]
pub struct RecordingTransport<T: Transport> {
    inner: T,
    entries: HashMap<String, serde_json::Value>,
}

impl<T: Transport> RecordingTransport<T> {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new(inner: T) -> Self {
        RecordingTransport {
            inner,
            entries: HashMap::new(),
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn into_fixture(self) -> FixtureFile {
        FixtureFile {
            fixture_version: crate::fixture::FIXTURE_VERSION,
            entries: self.entries,
        }
    }
}
impl<T: Transport> Transport for RecordingTransport<T> {
    fn deploy_contract(
        &mut self,
        wasm_path: &Path,
        source: &str,
        network: &str,
        package_name: &str,
    ) -> Result<String> {
        let result = self
            .inner
            .deploy_contract(wasm_path, source, network, package_name)?;
        let key = format!("deploy:{}", package_name);
        self.entries.insert(key, Value::String(result.clone()));
        Ok(result)
    }

    fn build_invoke_xdr(
        &mut self,
        contract_id: &str,
        source: &str,
        network: &str,
        function: &str,
        func_args: &[String],
        package: &str,
    ) -> Result<String> {
        let result = self.inner.build_invoke_xdr(
            contract_id,
            source,
            network,
            function,
            func_args,
            package,
        )?;
        let key = format!("invoke:{}:{}", package, function);
        self.entries.insert(key, Value::String(result.clone()));
        Ok(result)
    }

    fn simulate_transaction(
        &mut self,
        b64_xdr: &str,
        package: &str,
        function: &str,
    ) -> Result<Value> {
        let result = self
            .inner
            .simulate_transaction(b64_xdr, package, function)?;
        let key = format!("simulate:{}:{}", package, function);
        self.entries.insert(key, result.clone());
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::ReplayTransport;
    use serde_json::json;

    /// Deterministic in-memory transport standing in for the network.
    struct MockTransport;

    impl Transport for MockTransport {
        fn deploy_contract(
            &mut self,
            _wasm_path: &Path,
            _source: &str,
            _network: &str,
            package_name: &str,
        ) -> anyhow::Result<String> {
            Ok(format!("C{}", package_name))
        }

        fn build_invoke_xdr(
            &mut self,
            _contract_id: &str,
            _source: &str,
            _network: &str,
            function: &str,
            _func_args: &[String],
            _package: &str,
        ) -> anyhow::Result<String> {
            Ok(format!("XDR:{}", function))
        }

        fn simulate_transaction(
            &mut self,
            _b64_xdr: &str,
            _package: &str,
            function: &str,
        ) -> anyhow::Result<Value> {
            Ok(json!({ "result": { "ok": true, "fn": function } }))
        }
    }

    /// Record a run, persist the fixture, load it back, and replay it: every
    /// response must come back identical. This is the property that lets the
    /// crate be tested without touching testnet.
    #[test]
    fn record_then_replay_round_trip_preserves_responses() {
        let mut recording = RecordingTransport::new(MockTransport);
        let deploy_id = recording
            .deploy_contract(Path::new("c.wasm"), "alice", "testnet", "pkg")
            .unwrap();
        let xdr = recording
            .build_invoke_xdr("C1", "alice", "testnet", "do_work", &[], "pkg")
            .unwrap();
        let sim = recording
            .simulate_transaction(&xdr, "pkg", "do_work")
            .unwrap();

        let fixture = recording.into_fixture();

        // Serialize → load → replay.
        let tmp = tempfile::NamedTempFile::new().unwrap();
        fixture.save(tmp.path()).unwrap();
        let loaded = FixtureFile::load(tmp.path()).unwrap();

        let mut replay = ReplayTransport::new(loaded);
        assert_eq!(
            replay
                .deploy_contract(Path::new("c.wasm"), "alice", "testnet", "pkg")
                .unwrap(),
            deploy_id
        );
        assert_eq!(
            replay
                .build_invoke_xdr("C1", "alice", "testnet", "do_work", &[], "pkg")
                .unwrap(),
            xdr
        );
        assert_eq!(
            replay.simulate_transaction(&xdr, "pkg", "do_work").unwrap(),
            sim
        );
    }

    #[test]
    fn replay_reports_missing_entry() {
        let mut replay = ReplayTransport::new(FixtureFile::new());
        let err = replay
            .simulate_transaction("xdr", "pkg", "missing")
            .unwrap_err();
        assert!(err.to_string().contains("Fixture not found"));
    }

    #[test]
    fn fixture_rejects_wrong_version() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(
            tmp.path(),
            serde_json::to_string(&json!({
                "fixture_version": 999,
                "entries": {}
            }))
            .unwrap(),
        )
        .unwrap();
        let err = FixtureFile::load(tmp.path()).unwrap_err();
        assert!(err.to_string().contains("version mismatch"));
    }
}
