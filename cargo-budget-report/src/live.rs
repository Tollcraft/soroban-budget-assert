use crate::transport::Transport;
use anyhow::{Context, Result};
use serde_json::Value;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

/// Default Soroban RPC endpoint used for `simulateTransaction` when no
/// `--rpc-url` override is supplied.
pub const DEFAULT_RPC_URL: &str = "https://soroban-testnet.stellar.org:443";
pub const FUTURENET_RPC_URL: &str = "https://rpc-futurenet.stellar.org:443";
pub const MAINNET_RPC_URL: &str = "https://soroban.stellar.org:443";
pub const LOCAL_RPC_URL: &str = "http://localhost:8000/soroban/rpc";

/// A custom network target for local / standalone RPC nodes (#49).
///
/// When present, `simulateTransaction` is POSTed to `rpc_url` instead of the
/// public testnet endpoint, and the `stellar` deploy / invoke-build calls are
/// pointed at it with `--rpc-url` + `--network-passphrase` in place of
/// `--network <alias>`.
#[derive(Debug, Clone)]
pub struct NetworkOverride {
    /// RPC endpoint used for simulation and Stellar CLI operations.
    pub rpc_url: String,
    /// Network passphrase paired with [`Self::rpc_url`].
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
    network: Option<String>,
}

fn run_stellar_command(args: &[String], operation: &str) -> Result<String, crate::RetryFailure> {
    let output = Command::new("stellar").args(args).output().map_err(|e| {
        crate::RetryFailure::Permanent(format!(
            "failed to execute stellar-cli {}: {}",
            operation, e
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
}

/// Seconds curl may spend establishing the connection to the RPC endpoint.
const CURL_CONNECT_TIMEOUT_SECS: u64 = 30;
/// Seconds curl may spend on the whole simulate request. Without a bound a
/// silent endpoint hangs the run forever and the retry policy for timeouts
/// can never fire.
const CURL_MAX_TIME_SECS: u64 = 30;

fn run_simulation_request(endpoint: &str, payload: &Value) -> Result<Value, crate::RetryFailure> {
    run_simulation_request_with_timeouts(
        endpoint,
        payload,
        CURL_CONNECT_TIMEOUT_SECS,
        CURL_MAX_TIME_SECS,
    )
}

/// Argument list for the simulate POST, with bounded connect and total time.
fn simulation_curl_args(endpoint: &str, connect_timeout: u64, max_time: u64) -> Vec<String> {
    vec![
        "-s".into(),
        "-S".into(),
        "--connect-timeout".into(),
        connect_timeout.to_string(),
        "--max-time".into(),
        max_time.to_string(),
        "-X".into(),
        "POST".into(),
        "-H".into(),
        "Content-Type: application/json".into(),
        "-d".into(),
        "@-".into(),
        endpoint.into(),
    ]
}

fn run_simulation_request_with_timeouts(
    endpoint: &str,
    payload: &Value,
    connect_timeout: u64,
    max_time: u64,
) -> Result<Value, crate::RetryFailure> {
    let mut curl = Command::new("curl")
        .args(simulation_curl_args(endpoint, connect_timeout, max_time))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| crate::RetryFailure::Permanent(format!("failed to execute curl: {}", e)))?;

    {
        let stdin = curl
            .stdin
            .as_mut()
            .ok_or_else(|| crate::RetryFailure::Permanent("Failed to open stdin".to_string()))?;
        stdin
            .write_all(payload.to_string().as_bytes())
            .map_err(|e| {
                crate::RetryFailure::Permanent(format!("failed to write to stdin: {}", e))
            })?;
    }

    let output = curl.wait_with_output().map_err(|e| {
        crate::RetryFailure::Permanent(format!("failed to read curl output: {}", e))
    })?;

    if !output.status.success() {
        return Err(crate::RetryFailure::Transient(format!(
            "curl exited with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    serde_json::from_slice(&output.stdout)
        .map_err(|e| crate::RetryFailure::Transient(format!("Failed to parse RPC response: {}", e)))
}

impl LiveTransport {
    /// Creates a live transport using the supplied retry and output settings.
    ///
    /// When `net_override` is `Some`, all network operations target that
    /// endpoint and passphrase instead of a named Stellar network.
    pub fn new(
        retry_config: crate::RetryConfig,
        quiet: bool,
        net_override: Option<NetworkOverride>,
    ) -> Self {
        LiveTransport {
            retry_config,
            quiet,
            net_override,
            network: None,
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

    fn rpc_url(&self) -> Result<String> {
        let network = self.network.as_deref().unwrap_or("testnet");
        resolve_rpc_url(network, self.net_override.as_ref())
    }
}

pub fn resolve_rpc_url(network: &str, net_override: Option<&NetworkOverride>) -> Result<String> {
    if let Some(override_) = net_override {
        return Ok(override_.rpc_url.clone());
    }

    match network.trim().to_ascii_lowercase().as_str() {
        "testnet" | "test" => Ok(DEFAULT_RPC_URL.to_string()),
        "futurenet" | "future" => Ok(FUTURENET_RPC_URL.to_string()),
        "mainnet" | "pubnet" | "publicnet" | "public" => Ok(MAINNET_RPC_URL.to_string()),
        "local" | "localnet" | "standalone" => Ok(LOCAL_RPC_URL.to_string()),
        other => anyhow::bail!(
            "no built-in RPC endpoint is configured for network alias {:?}; pass --rpc-url and --network-passphrase",
            other
        ),
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
        self.network = Some(network.to_string());
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
                let args = [
                    "contract",
                    "deploy",
                    "--wasm",
                    &wasm_path_str,
                    "--source",
                    source,
                ]
                .into_iter()
                .map(String::from)
                .chain(net_args.clone())
                .collect::<Vec<_>>();
                run_stellar_command(&args, "deploy")
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
            || run_stellar_command(&invoke_args, "invoke"),
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
        let endpoint = self.rpc_url()?.to_string();

        crate::run_with_retry(
            &self.retry_config,
            self.quiet,
            "Simulate RPC request",
            || run_simulation_request(&endpoint, &rpc_payload),
            |last_error| {
                crate::Error::Message(format!("simulateTransaction RPC failed: {}", last_error))
            },
        )
        .map_err(anyhow::Error::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simulation_curl_args_bound_connect_and_total_time() {
        let args = simulation_curl_args("http://x", 7, 9);
        let pos = |flag: &str| args.iter().position(|a| a == flag).unwrap();
        assert_eq!(args[pos("--connect-timeout") + 1], "7");
        assert_eq!(args[pos("--max-time") + 1], "9");
        assert_eq!(CURL_CONNECT_TIMEOUT_SECS, 30);
        assert_eq!(CURL_MAX_TIME_SECS, 30);
    }

    /// An endpoint that accepts the connection but never answers must surface
    /// as a transient failure (so the retry policy applies) instead of hanging.
    #[test]
    fn silent_endpoint_times_out_as_transient_failure() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/", listener.local_addr().unwrap());
        let started = std::time::Instant::now();
        let result = run_simulation_request_with_timeouts(&endpoint, &serde_json::json!({}), 1, 1);
        assert!(started.elapsed() < std::time::Duration::from_secs(20));
        match result {
            Err(crate::RetryFailure::Transient(msg)) => assert!(msg.contains("curl exited")),
            other => panic!("expected transient failure, got {:?}", other.is_ok()),
        }
        drop(listener);
    }

    #[test]
    fn resolves_supported_network_aliases() {
        assert_eq!(resolve_rpc_url("testnet", None).unwrap(), DEFAULT_RPC_URL);
        assert_eq!(
            resolve_rpc_url("futurenet", None).unwrap(),
            FUTURENET_RPC_URL
        );
        assert_eq!(resolve_rpc_url("local", None).unwrap(), LOCAL_RPC_URL);
    }

    #[test]
    fn custom_rpc_override_has_precedence() {
        let override_ = NetworkOverride {
            rpc_url: "http://example.test/rpc".into(),
            network_passphrase: "custom".into(),
        };
        assert_eq!(
            resolve_rpc_url("futurenet", Some(&override_)).unwrap(),
            "http://example.test/rpc"
        );
    }

    #[test]
    fn unknown_network_alias_requires_an_explicit_rpc_url() {
        let error = resolve_rpc_url("private-net", None)
            .unwrap_err()
            .to_string();
        assert!(error.contains("no built-in RPC endpoint"));
        assert!(error.contains("--rpc-url"));
    }
}
