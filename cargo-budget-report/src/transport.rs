use crate::module_30::Result;
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

    fn simulate_transaction(
        &mut self,
        b64_xdr: &str,
        package: &str,
        function: &str,
    ) -> Result<Value>;
}
