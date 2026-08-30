use crate::transport::Transport;
use anyhow::{Context, Result};
use serde_json::Value;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

/// Default Soroban RPC endpoint used for `simulateTransaction` when no
/// `--rpc-url` override is supplied.
pub const DEFAULT_RPC_URL: &str = "https://soroban-testnet.stellar.org:443";

/// A custom network target for local / standalone RPC nodes (#49).
///
/// When present, `simulateTransaction` is POSTed to `rpc_url` instead of the
/// public testnet endpoint, and the `stellar` deploy / invoke-build calls are
/// pointed at it with `--rpc-url` + `--network-passphrase` in place of
/// `--network <alias>`.
#[derive(Debug, Clone)]
pub struct NetworkOverride {
    pub rpc_url: String,
    pub network_passphrase: String,
}

/// The production transport: runs `stellar` and `curl` against the real
/// network.
///
/// Retry policy lives here rather than in the callers because this is the
/// only implementation that talks to the network — transient failures
/// (rate limits, connection errors) are retried with the crate-wide
/// [`crate::run_with_retry`] machinery, while deterministic failures abort
/// immediately. [`crate::RetryConfig`] comes from `--max-retry-attempts` /
/// `--retry-backoff-secs` and the `[retry]` section of `budget.toml`.
#[derive(Clone, Debug)]
pub struct NetworkOverride {
    pub rpc_url: String,
    pub network_passphrase: String,
}

/// The production transport: runs `stellar` and `curl` against the real
/// network.
///
/// Retry policy lives here rather than in the callers because this is the
/// only implementation that talks to the network — transient failures
/// (rate limits, connection errors) are retried with the crate-wide
/// [`crate::run_with_retry`] machinery, while deterministic failures abort
/// immediately. [`crate::RetryConfig`] comes from `--max-retry-attempts` /
/// `--retry-backoff-secs` and the `[retry]` section of `budget.toml`.
pub struct LiveTransport {
    retry_config: crate::RetryConfig,
    quiet: bool,
    net_override: Option<NetworkOverride>,
}

impl LiveTransport {
    pub fn new(
        retry_config: crate::RetryConfig,
        quiet: bool,
        net_override: Option<NetworkOverride>,
    ) -> Self {
        LiveTransport {
            retry_config,
            quiet,
            net_override,
        }
    }

    /// `["--rpc-url", url, "--network-passphrase", phrase]` for a custom
    /// network, or `["--network", alias]` for a built-in one.
    fn network_cli_args<'a>(&'a self, network: &'a str) -> Vec<&'a str> {
        match &self.net_override {
            Some(o) => vec![
                "--rpc-url",
                &o.rpc_url,
                "--network-passphrase",
                &o.network_passphrase,
            ],
            None => vec!["--network", network],
        }
    }

    fn rpc_url(&self) -> &str {
        self.net_override
            .as_ref()
            .map(|o| o.rpc_url.as_str())
            .unwrap_or(DEFAULT_RPC_URL)
    }
}

impl Transport for LiveTransport {
    fn deploy_contract(
        &mut self,
        wasm_path: &Path,
        source: &str,
        network: &str,
        _package_name: &str,
    ) -> Result<String> {
        let wasm_path_str = wasm_path
            .to_str()
            .context("wasm path is not valid UTF-8")?
            .to_string();
        let net_args: Vec<String> = self
            .network_cli_args(network)
            .into_iter()
            .map(String::from)
            .collect();

        crate::run_with_retry(
            &self.retry_config,
            self.quiet,
            "Deploy",
            || {
                let mut cmd = Command::new("stellar");
                cmd.args([
                    "contract",
                    "deploy",
                    "--wasm",
                    &wasm_path_str,
                    "--source",
                    source,
                ]);
                cmd.args(&net_args);
                let output = cmd.output().map_err(|e| {
                    // A missing/unspawnable `stellar` binary is an
                    // environment problem, not something retry fixes.
                    crate::RetryFailure::Permanent(format!(
                        "failed to execute stellar-cli deploy: {}",
                        e
                    ))
                })?;

                if output.status.success() {
                    return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
                }

                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                if crate::is_transient_error(&stderr) {
                    Err(crate::RetryFailure::Transient(stderr))
                } else {
                    Err(crate::RetryFailure::Permanent(stderr))
                }
            },
            |last_error| {
                crate::Error::Message(format!("stellar contract deploy failed: {}", last_error))
            },
        )
        .map_err(anyhow::Error::from)
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
        let rpc_override = self
            .net_override
            .as_ref()
            .map(|o| (o.rpc_url.as_str(), o.network_passphrase.as_str()));
        let invoke_args = crate::build_invoke_args(
            contract_id,
            source,
            network,
            function,
            func_args,
            rpc_override,
        );

        crate::run_with_retry(
            &self.retry_config,
            self.quiet,
            "Invoke build",
            || {
                let output = Command::new("stellar")
                    .args(&invoke_args)
                    .output()
                    .map_err(|e| {
                        crate::RetryFailure::Permanent(format!(
                            "failed to execute stellar-cli invoke: {}",
                            e
                        ))
                    })?;

                if output.status.success() {
                    return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
                }

                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                if crate::is_transient_error(&stderr) {
                    Err(crate::RetryFailure::Transient(stderr))
                } else {
                    Err(crate::RetryFailure::Permanent(stderr))
                }
            },
            |last_error| crate::Error::Message(format!("stellar invoke failed: {}", last_error)),
        )
        .map_err(anyhow::Error::from)
    }

    fn simulate_transaction(
        &mut self,
        b64_xdr: &str,
        _package: &str,
        _function: &str,
    ) -> Result<Value> {
        let rpc_payload = crate::build_rpc_payload(b64_xdr);
        let endpoint = self.rpc_url().to_string();

        crate::run_with_retry(
            &self.retry_config,
            self.quiet,
            "Simulate RPC request",
            || {
                let mut curl = Command::new("curl")
                    .args([
                        "-s",
                        "-X",
                        "POST",
                        "-H",
                        "Content-Type: application/json",
                        "-d",
                        "@-",
                        endpoint.as_str(),
                    ])
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .spawn()
                    .map_err(|e| {
                        // A missing/unspawnable `curl` is an environment
                        // problem, not something a retry can fix.
                        crate::RetryFailure::Permanent(format!("failed to execute curl: {}", e))
                    })?;

                {
                    let stdin = curl.stdin.as_mut().ok_or_else(|| {
                        crate::RetryFailure::Permanent("Failed to open stdin".to_string())
                    })?;
                    stdin
                        .write_all(rpc_payload.to_string().as_bytes())
                        .map_err(|e| {
                            crate::RetryFailure::Permanent(format!(
                                "failed to write to stdin: {}",
                                e
                            ))
                        })?;
                }

                let curl_output = curl.wait_with_output().map_err(|e| {
                    crate::RetryFailure::Permanent(format!("failed to read curl output: {}", e))
                })?;

                if !curl_output.status.success() {
                    // Connection refused, DNS failure, TLS errors,
                    // HTTP-level failures surfaced by `curl -s`: all
                    // plausibly transient.
                    return Err(crate::RetryFailure::Transient(format!(
                        "curl exited with status {}: {}",
                        curl_output.status,
                        String::from_utf8_lossy(&curl_output.stderr).trim()
                    )));
                }

                serde_json::from_slice(&curl_output.stdout).map_err(|e| {
                    // An empty or truncated body almost always means the
                    // connection dropped mid-response; treat it as transient.
                    crate::RetryFailure::Transient(format!("Failed to parse RPC response: {}", e))
                })
            },
            |last_error| {
                crate::Error::Message(format!("simulateTransaction RPC failed: {}", last_error))
            },
        )
        .map_err(anyhow::Error::from)
    }
}
