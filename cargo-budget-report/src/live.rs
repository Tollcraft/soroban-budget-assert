use crate::transport::Transport;
use crate::module_30::{Context, Error, Result};
use serde_json::Value;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

pub struct LiveTransport;

impl Transport for LiveTransport {
    fn deploy_contract(
        &mut self,
        wasm_path: &Path,
        source: &str,
        network: &str,
        _package_name: &str,
    ) -> Result<String> {
        let output = Command::new("stellar")
            .args([
                "contract",
                "deploy",
                "--wasm",
                wasm_path.to_str().context("wasm path is not valid UTF-8")?,
                "--source",
                source,
                "--network",
                network,
            ])
            .output()
            .context("failed to execute stellar-cli deploy")?;

        if output.status.success() {
            let contract_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Ok(contract_id)
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(Error::Message(format!("stellar contract deploy failed: {}", stderr)))
        }
    }

    fn build_invoke_xdr(
        &mut self,
        contract_id: &str,
        source: &str,
        network: &str,
        function: &str,
        func_args: &[String],
        _package: &str,
    ) -> Result<String> {
        let invoke_args =
            crate::build_invoke_args(contract_id, source, network, function, func_args);
        let output = Command::new("stellar")
            .args(&invoke_args)
            .output()
            .context("failed to execute stellar-cli invoke")?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(Error::Message(format!("stellar invoke failed: {}", stderr)))
        }
    }

    fn simulate_transaction(
        &mut self,
        b64_xdr: &str,
        _package: &str,
        _function: &str,
    ) -> Result<Value> {
        let rpc_payload = crate::build_rpc_payload(b64_xdr);

        let mut curl = Command::new("curl")
            .args([
                "-s",
                "-X",
                "POST",
                "-H",
                "Content-Type: application/json",
                "-d",
                "@-",
                "https://soroban-testnet.stellar.org:443",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .context("failed to execute curl")?;

        {
            let stdin = curl.stdin.as_mut().context("Failed to open stdin")?;
            stdin
                .write_all(rpc_payload.to_string().as_bytes())
                .context("Failed to write to stdin")?;
        }

        let curl_output = curl
            .wait_with_output()
            .context("Failed to read curl output")?;
        serde_json::from_slice(&curl_output.stdout).context("Failed to parse RPC response")
    }
}
