use crate::fixture::FixtureFile;
use crate::transport::Transport;
use crate::module_30::Result;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

pub struct RecordingTransport<T: Transport> {
    inner: T,
    entries: HashMap<String, serde_json::Value>,
}

impl<T: Transport> RecordingTransport<T> {
    pub fn new(inner: T) -> Self {
        RecordingTransport {
            inner,
            entries: HashMap::new(),
        }
    }

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
