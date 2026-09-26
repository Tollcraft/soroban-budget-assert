//! Record mode for the [`Transport`] layer.
//!
//! [`RecordingTransport`] sits between the report pipeline and a real
//! transport, forwarding every call unchanged and keeping a copy of each
//! successful response. The copies are keyed exactly the way
//! [`crate::replay::ReplayTransport`] looks them up (see
//! [`crate::transport::deploy_key`] and friends), so a recorded run saved as a
//! [`FixtureFile`] can later be replayed offline, response for response.
//!
//! Two properties callers can rely on:
//!
//! * **Errors are not recorded.** A failed inner call is returned as-is and
//!   leaves the fixture untouched, so a replay of that key reports
//!   "Fixture not found" instead of serving a stale or partial value.
//! * **Last write wins.** Keys are per package / function, not per call, so
//!   repeating a call overwrites the earlier response.

use crate::fixture::FixtureFile;
use crate::transport::{deploy_key, invoke_key, simulate_key, Transport};
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
    /// The transport that actually does the work; every call is forwarded.
    inner: T,
    /// Captured responses, keyed by the fixture keys from
    /// [`crate::transport`]. Becomes [`FixtureFile::entries`] verbatim.
    entries: HashMap<String, serde_json::Value>,
}

impl<T: Transport> RecordingTransport<T> {
    /// Wrap `inner`, starting with an empty recording.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new(inner: T) -> Self {
        RecordingTransport {
            inner,
            entries: HashMap::new(),
        }
    }

    /// Finish recording and turn the captured responses into a fixture
    /// stamped with the current [`crate::fixture::FIXTURE_VERSION`].
    ///
    /// Consumes the recorder: the inner transport is dropped here, and the
    /// entry map moves into the fixture without being copied.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn into_fixture(self) -> FixtureFile {
        FixtureFile {
            fixture_version: crate::fixture::FIXTURE_VERSION,
            entries: self.entries,
        }
    }
}

/// Each method follows the same shape: forward to `inner`, bail out early on
/// error (the `?` means nothing is recorded), then store a clone of the
/// response under its fixture key and hand the original back to the caller.
/// The clone is unavoidable — the fixture and the caller each need to own a
/// copy.
impl<T: Transport> Transport for RecordingTransport<T> {
    /// Forward the deploy and record the returned contract ID under
    /// `deploy:<package_name>`. The WASM path, source, and network are not
    /// part of the key; replay only needs the package to find the ID.
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
        let key = deploy_key(package_name);
        self.entries.insert(key, Value::String(result.clone()));
        Ok(result)
    }

    /// Forward the XDR build and record the base64 envelope under
    /// `invoke:<package>:<function>`. Arguments are not part of the key, so
    /// two invocations of one function with different args share an entry.
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
        let key = invoke_key(package, function);
        self.entries.insert(key, Value::String(result.clone()));
        Ok(result)
    }

    /// Forward the simulation and record the raw RPC JSON response under
    /// `simulate:<package>:<function>`. The XDR is not part of the key; the
    /// package / function pair already identifies the simulated call.
    fn simulate_transaction(
        &mut self,
        b64_xdr: &str,
        package: &str,
        function: &str,
    ) -> Result<Value> {
        let result = self
            .inner
            .simulate_transaction(b64_xdr, package, function)?;
        let key = simulate_key(package, function);
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

    /// Transport whose every call fails, to check nothing gets recorded.
    struct FailingTransport;

    impl Transport for FailingTransport {
        fn deploy_contract(
            &mut self,
            _wasm_path: &Path,
            _source: &str,
            _network: &str,
            _package_name: &str,
        ) -> anyhow::Result<String> {
            anyhow::bail!("deploy failed")
        }

        fn build_invoke_xdr(
            &mut self,
            _contract_id: &str,
            _source: &str,
            _network: &str,
            _function: &str,
            _func_args: &[String],
            _package: &str,
        ) -> anyhow::Result<String> {
            anyhow::bail!("invoke failed")
        }

        fn simulate_transaction(
            &mut self,
            _b64_xdr: &str,
            _package: &str,
            _function: &str,
        ) -> anyhow::Result<Value> {
            anyhow::bail!("simulate failed")
        }
    }

    #[test]
    fn failed_inner_calls_are_propagated_and_not_recorded() {
        let mut recording = RecordingTransport::new(FailingTransport);
        let deploy = recording
            .deploy_contract(Path::new("c.wasm"), "alice", "testnet", "pkg")
            .unwrap_err();
        assert_eq!(deploy.to_string(), "deploy failed");
        let invoke = recording
            .build_invoke_xdr("C1", "alice", "testnet", "f", &[], "pkg")
            .unwrap_err();
        assert_eq!(invoke.to_string(), "invoke failed");
        let simulate = recording
            .simulate_transaction("xdr", "pkg", "f")
            .unwrap_err();
        assert_eq!(simulate.to_string(), "simulate failed");

        assert!(recording.into_fixture().entries.is_empty());
    }

    #[test]
    fn repeated_call_overwrites_the_earlier_response() {
        let mut recording = RecordingTransport::new(MockTransport);
        recording.simulate_transaction("x1", "pkg", "f").unwrap();
        let second = recording.simulate_transaction("x2", "pkg", "f").unwrap();
        let entries = recording.into_fixture().entries;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries["simulate:pkg:f"], second);
    }

    #[test]
    fn empty_recording_yields_current_version_fixture() {
        let fixture = RecordingTransport::new(MockTransport).into_fixture();
        assert_eq!(fixture.fixture_version, crate::fixture::FIXTURE_VERSION);
        assert!(fixture.entries.is_empty());
    }

    #[test]
    fn replay_missing_entry_names_the_full_key() {
        let mut replay = ReplayTransport::new(FixtureFile::new());
        let deploy = replay
            .deploy_contract(Path::new("c.wasm"), "alice", "testnet", "pkg")
            .unwrap_err();
        assert_eq!(deploy.to_string(), "Fixture not found for deploy:pkg");
        let invoke = replay
            .build_invoke_xdr("C1", "alice", "testnet", "f", &[], "pkg")
            .unwrap_err();
        assert_eq!(invoke.to_string(), "Fixture not found for invoke:pkg:f");
        let simulate = replay.simulate_transaction("xdr", "pkg", "f").unwrap_err();
        assert_eq!(simulate.to_string(), "Fixture not found for simulate:pkg:f");
    }

    #[test]
    fn recording_stores_entries_under_transport_keys() {
        let mut recording = RecordingTransport::new(MockTransport);
        recording
            .deploy_contract(Path::new("c.wasm"), "alice", "testnet", "pkg")
            .unwrap();
        recording
            .build_invoke_xdr("C1", "alice", "testnet", "f", &[], "pkg")
            .unwrap();
        recording.simulate_transaction("x", "pkg", "f").unwrap();
        let entries = recording.into_fixture().entries;
        assert_eq!(entries.len(), 3);
        assert!(entries.contains_key(&crate::transport::deploy_key("pkg")));
        assert!(entries.contains_key(&crate::transport::invoke_key("pkg", "f")));
        assert!(entries.contains_key(&crate::transport::simulate_key("pkg", "f")));
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
