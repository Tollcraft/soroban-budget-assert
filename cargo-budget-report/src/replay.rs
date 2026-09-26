use crate::fixture::FixtureFile;
use crate::transport::{deploy_key, invoke_key, simulate_key, Transport};
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
/// Not reachable from a CLI flag yet; the tests in `record.rs` are the
/// current consumers, and a record/replay flag is the follow-up that
/// promotes it to a user-facing path.
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
}

impl Transport for ReplayTransport {
    fn deploy_contract(
        &mut self,
        _wasm_path: &Path,
        _source: &str,
        _network: &str,
        package_name: &str,
    ) -> Result<String> {
        // The deploy response is a plain contract-id string, so `.as_str()`
        // is the correct extractor.  A missing key means the fixture was
        // recorded without this package, which is a test-setup error.
        let key = deploy_key(package_name);
        self.entries
            .get(&key)
            .and_then(|v| v.as_str().map(String::from))
            .with_context(|| format!("Fixture not found for {key}"))
    }

    fn build_invoke_xdr(
        &mut self,
        _contract_id: &str,
        _source: &str,
        _network: &str,
        function: &str,
        _func_args: &[String],
        package: &str,
    ) -> Result<String> {
        // The invoke response is an XDR string (base-64 encoded transaction
        // envelope).  The key encodes both the package and the function name
        // so different entry points on the same contract are stored separately.
        let key = invoke_key(package, function);
        self.entries
            .get(&key)
            .and_then(|v| v.as_str().map(String::from))
            .with_context(|| format!("Fixture not found for {key}"))
    }

    fn simulate_transaction(
        &mut self,
        _b64_xdr: &str,
        package: &str,
        function: &str,
    ) -> Result<Value> {
        // The simulate response is a full JSON object (the RPC SimulateTransaction
        // result), so `.cloned()` is used rather than `.as_str()` to preserve
        // the entire structure for downstream budget extraction.
        let key = simulate_key(package, function);
        self.entries
            .get(&key)
            .cloned()
            .with_context(|| format!("Fixture not found for {key}"))
    }
}
