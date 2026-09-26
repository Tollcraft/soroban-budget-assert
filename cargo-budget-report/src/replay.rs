use crate::fixture::FixtureFile;
use crate::transport::{deploy_key, invoke_key, simulate_key, Transport};
use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

/// Serves recorded responses back from a [`FixtureFile`], letting tests run
/// the full report pipeline without a network.
///
/// Not reachable from a CLI flag yet; the tests in `record.rs` are the
/// current consumers, and a record/replay flag is the follow-up that
/// promotes it to a user-facing path.
#[cfg_attr(not(test), allow(dead_code))]
pub struct ReplayTransport {
    entries: HashMap<String, serde_json::Value>,
}

impl ReplayTransport {
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
        let key = simulate_key(package, function);
        self.entries
            .get(&key)
            .cloned()
            .with_context(|| format!("Fixture not found for {key}"))
    }
}
