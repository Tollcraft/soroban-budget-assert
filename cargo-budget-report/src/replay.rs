use crate::fixture::FixtureFile;
use crate::transport::Transport;
use crate::module_30::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

pub struct ReplayTransport {
    entries: HashMap<String, serde_json::Value>,
}

impl ReplayTransport {
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
        let key = format!("deploy:{}", package_name);
        self.entries
            .get(&key)
            .and_then(|v| v.as_str().map(String::from))
            .with_context(|| format!("Fixture not found for deploy:{}", package_name))
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
        let key = format!("invoke:{}:{}", package, function);
        self.entries
            .get(&key)
            .and_then(|v| v.as_str().map(String::from))
            .with_context(|| format!("Fixture not found for invoke:{}:{}", package, function))
    }

    fn simulate_transaction(
        &mut self,
        _b64_xdr: &str,
        package: &str,
        function: &str,
    ) -> Result<Value> {
        let key = format!("simulate:{}:{}", package, function);
        self.entries
            .get(&key)
            .cloned()
            .with_context(|| format!("Fixture not found for simulate:{}:{}", package, function))
    }
}
