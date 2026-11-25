use crate::transport::Transport;
use anyhow::{Context, Result};
use serde_json::Value;
use std::fmt;
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

impl NetworkOverride {
    /// Validates the custom endpoint and returns it with surrounding
    /// whitespace removed.
    ///
    /// `--rpc-url` is handed verbatim to `curl` and to the Stellar CLI, so an
    /// unchecked value either fails late with an opaque transport error or —
    /// worse — sends the signed transaction envelope over an unencrypted
    /// `http://` connection to a remote host. Every live path resolves its
    /// endpoint through this method, so `simulateTransaction`, `stellar
    /// contract deploy`, and `stellar contract invoke --build-only` all agree
    /// on what is acceptable.
    ///
    /// The accepted shape is documented on [`validate_rpc_endpoint`].
    fn validated_rpc_url(&self) -> Result<&str> {
        validate_rpc_endpoint(&self.rpc_url)
            .map_err(|e| anyhow::anyhow!("invalid --rpc-url {:?}: {}", self.rpc_url, e))
    }
}

/// Hosts treated as loopback: traffic never leaves the machine, so plaintext
/// `http://` exposes nothing on the wire.
const LOOPBACK_HOSTS: [&str; 3] = ["localhost", "127.0.0.1", "::1"];

/// Why a user-supplied `--rpc-url` value was rejected.
///
/// Each variant carries enough context (the offending scheme, host, port, or
/// authority) for the CLI to name the exact problem. Keeping this a typed
/// enum rather than a formatted string lets the unit tests pin each rule
/// without matching on prose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpcUrlError {
    /// The value was empty or whitespace-only.
    Empty,
    /// No `scheme://` prefix at all, e.g. `rpc.example.com/soroban/rpc`.
    MissingScheme,
    /// A scheme other than `http` / `https` (includes `HTTPS` only after
    /// case-folding, which is what the matcher does).
    UnsupportedScheme(String),
    /// A scheme with no authority, e.g. `https:///rpc`.
    MissingHost,
    /// Userinfo (`user[:password]@`) embedded in the authority.
    EmbeddedCredentials,
    /// Plaintext `http://` aimed at a host that is not loopback.
    RemotePlaintext(String),
    /// An authority that is not a well-formed `host[:port]`.
    InvalidAuthority(String),
    /// A port that is not an integer in `1..=65535`.
    InvalidPort(String),
}

impl fmt::Display for RpcUrlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "the URL is empty"),
            Self::MissingScheme => write!(
                f,
                "missing URI scheme; expected `https://host/...` (or `http://` for a loopback host)"
            ),
            Self::UnsupportedScheme(scheme) => write!(
                f,
                "unsupported scheme `{scheme}`; only `https` is accepted for remote endpoints (`http` is allowed for loopback hosts)"
            ),
            Self::MissingHost => write!(f, "missing host after the scheme"),
            Self::EmbeddedCredentials => write!(
                f,
                "embedded credentials (`user[:password]@host`) are not allowed; pass the endpoint without userinfo"
            ),
            Self::RemotePlaintext(host) => write!(
                f,
                "unencrypted `http://` is only allowed for loopback hosts (localhost, 127.0.0.1, [::1]), not `{host}`; use `https://`"
            ),
            Self::InvalidAuthority(authority) => {
                write!(f, "`{authority}` is not a valid host[:port] authority")
            }
            Self::InvalidPort(port) => write!(
                f,
                "invalid port `{port}`; expected an integer between 1 and 65535"
            ),
        }
    }
}

impl std::error::Error for RpcUrlError {}

/// Returns `true` for hosts whose traffic cannot leave the machine.
///
/// Matching is case-insensitive and tolerates a trailing dot (the FQDN
/// spelling of `localhost.`). Only the literal loopback spellings are
/// accepted: `0.0.0.0` is a bind address, not a loopback destination, and
/// the rest of `127.0.0.0/8` is not treated as local by this tool.
fn is_loopback_host(host: &str) -> bool {
    let normalized = host.trim_end_matches('.').to_ascii_lowercase();
    LOOPBACK_HOSTS.contains(&normalized.as_str())
}

/// Splits an authority into its host and optional port, accepting a
/// bracketed IPv6 literal.
///
/// An unbracketed authority containing `:` is rejected rather than guessed
/// at: `::1` is ambiguous with a `host:port` pair, and RFC 3986 requires
/// brackets around IPv6 literals anyway.
fn split_authority(authority: &str) -> Result<(&str, Option<&str>), RpcUrlError> {
    if let Some(rest) = authority.strip_prefix('[') {
        let Some(close) = rest.find(']') else {
            return Err(RpcUrlError::InvalidAuthority(authority.to_string()));
        };
        let host = &rest[..close];
        if host.is_empty() {
            return Err(RpcUrlError::InvalidAuthority(authority.to_string()));
        }
        let after = &rest[close + 1..];
        if after.is_empty() {
            return Ok((host, None));
        }
        let Some(port) = after.strip_prefix(':') else {
            return Err(RpcUrlError::InvalidAuthority(authority.to_string()));
        };
        return Ok((host, Some(port)));
    }

    match authority.rsplit_once(':') {
        Some((host, port)) => {
            // A second `:` means an unbracketed IPv6 literal (or something
            // equally malformed), not a port separator.
            if host.is_empty() || host.contains(':') {
                return Err(RpcUrlError::InvalidAuthority(authority.to_string()));
            }
            Ok((host, Some(port)))
        }
        None => Ok((authority, None)),
    }
}

/// Returns `true` when `host` only contains characters allowed in an RFC 3986
/// reg-name (`ALPHA` / `DIGIT` / `-` / `.` / `_` / `~`) or in an IPv6 literal
/// (`:`). Rejecting everything else keeps whitespace, globs, and shell
/// metacharacters out of a value that is handed to `curl` and `stellar`.
fn is_valid_host(host: &str) -> bool {
    if host.is_empty() {
        return false;
    }
    for c in host.chars() {
        let allowed = c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_' | '~' | ':');
        if !allowed {
            return false;
        }
    }
    true
}

/// Validates a custom `--rpc-url` and returns it trimmed of surrounding
/// whitespace.
///
/// The rules mirror what the live transport actually needs:
///
/// * A `scheme://` prefix is mandatory, and the scheme must be `https` —
///   except for loopback hosts (`localhost`, `127.0.0.1`, `[::1]`), where
///   `http://` is allowed because the transaction envelope never leaves the
///   machine.
/// * Userinfo (`user:password@host`) is rejected outright: `curl` would send
///   it as a basic-auth header, and it usually ends up in shell history and
///   logs.
/// * The authority must be a non-empty, well-formed `host[:port]`; a port
///   must be a number in `1..=65535`.
///
/// The value is returned with only surrounding whitespace removed; no other
/// rewriting happens, so the URL the user supplied is the URL that is used.
///
/// # Examples
///
/// ```text
/// https://soroban-testnet.stellar.org:443  -> Ok
/// http://localhost:8000/soroban/rpc        -> Ok (loopback)
/// http://rpc.example.com/soroban/rpc       -> Err(RemotePlaintext)
/// https://user:pass@rpc.example.com        -> Err(EmbeddedCredentials)
/// rpc.example.com/soroban/rpc              -> Err(MissingScheme)
/// ```
fn validate_rpc_endpoint(url: &str) -> Result<&str, RpcUrlError> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err(RpcUrlError::Empty);
    }

    // Split on the first `://` so a path containing `://` cannot be mistaken
    // for the scheme separator. `split_once` also rejects bare schemes
    // (`https:example.com`) rather than treating the whole value as a host.
    let Some((scheme, rest)) = trimmed.split_once("://") else {
        return Err(RpcUrlError::MissingScheme);
    };
    let https = match scheme.to_ascii_lowercase().as_str() {
        "https" => true,
        "http" => false,
        other => return Err(RpcUrlError::UnsupportedScheme(other.to_string())),
    };

    // The authority ends at the first path, query, or fragment marker.
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    if authority.is_empty() {
        return Err(RpcUrlError::MissingHost);
    }
    // Checked before host/port parsing so `https://user:pass@host` reports
    // the credential problem, not a malformed port.
    if authority.contains('@') {
        return Err(RpcUrlError::EmbeddedCredentials);
    }

    let (host, port) = split_authority(authority)?;
    if !is_valid_host(host) {
        return Err(RpcUrlError::InvalidAuthority(authority.to_string()));
    }
    if let Some(port) = port {
        if port.is_empty() || !port.bytes().all(|b| b.is_ascii_digit()) {
            return Err(RpcUrlError::InvalidPort(port.to_string()));
        }
        // `u16` parsing rejects anything above 65535; `0` is not a usable
        // endpoint port either.
        match port.parse::<u16>() {
            Ok(n) if n > 0 => {}
            _ => return Err(RpcUrlError::InvalidPort(port.to_string())),
        }
    }

    if !https && !is_loopback_host(host) {
        return Err(RpcUrlError::RemotePlaintext(host.to_string()));
    }

    Ok(trimmed)
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

/// How long `curl` may spend establishing the connection before the request
/// is abandoned (#825).
const CONNECT_TIMEOUT_SECS: u64 = 30;

/// How long `curl` may spend on the whole request before the request is
/// abandoned (#825).
///
/// Without a bound `curl` never gives up: an endpoint that accepts the TCP
/// connection and then says nothing leaves the measurement loop stalled with
/// no output and no exit status, so the retry policy documented for
/// "connection errors and timeouts" can never fire on a hang. A bounded wait
/// makes the hang a non-zero `curl` exit, which the existing
/// [`crate::RetryFailure::Transient`] classification below already retries.
///
/// Both values match the `timeout_secs` the deleted `config` module advertised
/// (it was read by nothing). They are deliberately not user-configurable: a
/// flag would need a CLI-reference row, which another issue owns.
const MAX_TIME_SECS: u64 = 30;

/// Builds the `curl` argument list for one `simulateTransaction` POST.
///
/// The endpoint is last so the `-d @-` (read the body from stdin) pair cannot
/// be mistaken for it by a reader scanning the list.
fn build_curl_args(endpoint: &str, connect_timeout_secs: u64, max_time_secs: u64) -> Vec<String> {
    vec![
        "-s".to_string(),
        "--connect-timeout".to_string(),
        connect_timeout_secs.to_string(),
        "--max-time".to_string(),
        max_time_secs.to_string(),
        "-X".to_string(),
        "POST".to_string(),
        "-H".to_string(),
        "Content-Type: application/json".to_string(),
        "-d".to_string(),
        "@-".to_string(),
        endpoint.to_string(),
    ]
}

fn run_simulation_request(endpoint: &str, payload: &Value) -> Result<Value, crate::RetryFailure> {
    run_simulation_request_with_timeouts(endpoint, payload, CONNECT_TIMEOUT_SECS, MAX_TIME_SECS)
}

fn run_simulation_request_with_timeouts(
    endpoint: &str,
    payload: &Value,
    connect_timeout_secs: u64,
    max_time_secs: u64,
) -> Result<Value, crate::RetryFailure> {
    let args = build_curl_args(endpoint, connect_timeout_secs, max_time_secs);
    let mut curl = Command::new("curl")
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
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
    fn network_cli_args<'a>(&'a self, network: &'a str) -> Result<Vec<&'a str>> {
        match &self.net_override {
            Some(o) => Ok(vec![
                "--rpc-url",
                o.validated_rpc_url()?,
                "--network-passphrase",
                &o.network_passphrase,
            ]),
            None => Ok(vec!["--network", network]),
        }
    }

    fn rpc_url(&self) -> Result<String> {
        let network = self.network.as_deref().unwrap_or("testnet");
        resolve_rpc_url(network, self.net_override.as_ref())
    }
}

pub fn resolve_rpc_url(network: &str, net_override: Option<&NetworkOverride>) -> Result<String> {
    if let Some(override_) = net_override {
        // A user-supplied endpoint is validated before it is returned: this
        // is the earliest point every caller (simulation, watch mode, deploy)
        // passes through, and the error carries the offending URL so the CLI
        // can say exactly which value to fix.
        return Ok(override_.validated_rpc_url()?.to_string());
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
            .network_cli_args(network)?
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
        let rpc_override = match self.net_override.as_ref() {
            Some(o) => Some((o.validated_rpc_url()?, o.network_passphrase.as_str())),
            None => None,
        };
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
        _source: &str,
        _network: &str,
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
    use std::net::TcpListener;
    use std::time::Duration;

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
            rpc_url: "https://example.test/rpc".into(),
            network_passphrase: "custom".into(),
        };
        assert_eq!(
            resolve_rpc_url("futurenet", Some(&override_)).unwrap(),
            "https://example.test/rpc"
        );
    }

    #[test]
    fn custom_rpc_override_is_trimmed_before_use() {
        let override_ = NetworkOverride {
            rpc_url: "  https://example.test/rpc\n".into(),
            network_passphrase: "custom".into(),
        };
        assert_eq!(
            resolve_rpc_url("testnet", Some(&override_)).unwrap(),
            "https://example.test/rpc"
        );
    }

    #[test]
    fn custom_rpc_override_rejects_plaintext_remote_endpoint() {
        let override_ = NetworkOverride {
            rpc_url: "http://example.test/rpc".into(),
            network_passphrase: "custom".into(),
        };
        let error = resolve_rpc_url("testnet", Some(&override_))
            .unwrap_err()
            .to_string();
        assert!(error.contains("invalid --rpc-url"), "{error}");
        assert!(error.contains("http://example.test/rpc"), "{error}");
        assert!(error.contains("https://"), "{error}");
    }

    #[test]
    fn custom_rpc_override_allows_plaintext_loopback_endpoint() {
        let override_ = NetworkOverride {
            rpc_url: "http://127.0.0.1:8000/soroban/rpc".into(),
            network_passphrase: "custom".into(),
        };
        assert_eq!(
            resolve_rpc_url("testnet", Some(&override_)).unwrap(),
            "http://127.0.0.1:8000/soroban/rpc"
        );
    }

    #[test]
    fn custom_rpc_override_rejects_embedded_credentials() {
        let override_ = NetworkOverride {
            rpc_url: "https://user:secret@example.test/rpc".into(),
            network_passphrase: "custom".into(),
        };
        let error = resolve_rpc_url("testnet", Some(&override_))
            .unwrap_err()
            .to_string();
        assert!(error.contains("invalid --rpc-url"), "{error}");
        assert!(error.contains("embedded credentials"), "{error}");
    }

    #[test]
    fn unknown_network_alias_requires_an_explicit_rpc_url() {
        let error = resolve_rpc_url("private-net", None)
            .unwrap_err()
            .to_string();
        assert!(error.contains("no built-in RPC endpoint"));
        assert!(error.contains("--rpc-url"));
    }

    // -- #825: the simulate POST must not be able to hang forever ----------

    #[test]
    fn simulation_curl_args_bound_the_wait() {
        let args = build_curl_args("http://127.0.0.1:1/rpc", 11, 22);

        let flag_value = |flag: &str| -> Option<&str> {
            args.iter()
                .position(|a| a == flag)
                .map(|i| args[i + 1].as_str())
        };
        assert_eq!(flag_value("--connect-timeout"), Some("11"));
        assert_eq!(flag_value("--max-time"), Some("22"));

        // The endpoint is positional, so it must stay the final argument and
        // the body must still be read from stdin.
        assert_eq!(
            args.last().map(String::as_str),
            Some("http://127.0.0.1:1/rpc")
        );
        let body_at = args.iter().position(|a| a == "-d").expect("-d present");
        assert_eq!(args[body_at + 1], "@-");

        let defaults = build_curl_args(DEFAULT_RPC_URL, CONNECT_TIMEOUT_SECS, MAX_TIME_SECS);
        // The shipped defaults are the 30 s the deleted `config` module
        // promised, so removing that module did not lower the guarantee.
        assert!(defaults.contains(&"30".to_string()), "{defaults:?}");
    }

    #[test]
    fn a_silent_endpoint_is_classified_transient() {
        // Accept the connection and then say nothing: exactly the hang that
        // used to stall the measurement loop with no error at all.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let accepted = std::thread::spawn(move || {
            let (_stream, _addr) = listener.accept().expect("curl connects");
            std::thread::sleep(Duration::from_secs(30));
        });

        let payload =
            serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "simulateTransaction"});
        let outcome = run_simulation_request_with_timeouts(&endpoint, &payload, 5, 1);
        accepted.join().ok();

        match outcome {
            Err(crate::RetryFailure::Transient(message)) => {
                assert!(
                    message.contains("curl exited with status"),
                    "expected a bounded-wait curl failure, got: {message}"
                );
            }
            Err(crate::RetryFailure::Permanent(message)) => {
                panic!("a timeout must be retryable, but it was classified permanent: {message}")
            }
            Ok(value) => panic!("a silent endpoint must not yield a response: {value}"),
        }
    }

    mod rpc_endpoint {
        use super::*;

        #[test]
        fn accepts_https_remote_endpoints() {
            let url = "https://soroban-testnet.stellar.org:443";
            assert_eq!(validate_rpc_endpoint(url), Ok(url));
        }

        #[test]
        fn accepts_the_built_in_default_endpoints() {
            // `LOCAL_RPC_URL` is plaintext over loopback, so it must keep
            // passing: rejecting the built-in `local` alias would be a
            // regression in the local-testing workflow.
            for url in [
                "https://soroban-testnet.stellar.org:443",
                "https://rpc-futurenet.stellar.org:443",
                "https://soroban.stellar.org:443",
                "http://localhost:8000/soroban/rpc",
            ] {
                assert_eq!(validate_rpc_endpoint(url), Ok(url), "{url}");
            }
        }

        #[test]
        fn accepts_plaintext_loopback_hosts() {
            for url in [
                "http://localhost:8000/soroban/rpc",
                "http://LOCALHOST/soroban/rpc",
                "http://localhost./soroban/rpc",
                "http://127.0.0.1:8000/soroban/rpc",
                "http://[::1]:8000/soroban/rpc",
            ] {
                assert!(validate_rpc_endpoint(url).is_ok(), "{url}");
            }
        }

        #[test]
        fn rejects_plaintext_remote_hosts() {
            assert_eq!(
                validate_rpc_endpoint("http://rpc.example.test/soroban/rpc"),
                Err(RpcUrlError::RemotePlaintext("rpc.example.test".into()))
            );
            // A bind address is not a loopback destination.
            assert_eq!(
                validate_rpc_endpoint("http://0.0.0.0:8000/soroban/rpc"),
                Err(RpcUrlError::RemotePlaintext("0.0.0.0".into()))
            );
        }

        #[test]
        fn rejects_embedded_credentials_even_over_https_and_loopback() {
            for url in [
                "https://user:pass@rpc.example.test/soroban/rpc",
                "https://user@rpc.example.test/soroban/rpc",
                "http://user:pass@localhost:8000/soroban/rpc",
            ] {
                assert_eq!(
                    validate_rpc_endpoint(url),
                    Err(RpcUrlError::EmbeddedCredentials),
                    "{url}"
                );
            }
        }

        #[test]
        fn rejects_empty_values() {
            for url in ["", "   ", "\n\t"] {
                assert_eq!(
                    validate_rpc_endpoint(url),
                    Err(RpcUrlError::Empty),
                    "{url:?}"
                );
            }
        }

        #[test]
        fn rejects_values_without_a_recognisable_scheme() {
            assert_eq!(
                validate_rpc_endpoint("rpc.example.test"),
                Err(RpcUrlError::MissingScheme)
            );
            assert_eq!(
                validate_rpc_endpoint("rpc.example.test:443/soroban/rpc"),
                Err(RpcUrlError::MissingScheme)
            );
            assert_eq!(
                validate_rpc_endpoint("https:/rpc.example.test"),
                Err(RpcUrlError::MissingScheme)
            );
        }

        #[test]
        fn rejects_other_schemes() {
            assert_eq!(
                validate_rpc_endpoint("ftp://rpc.example.test/rpc"),
                Err(RpcUrlError::UnsupportedScheme("ftp".into()))
            );
            assert_eq!(
                validate_rpc_endpoint("ws://localhost:8000/soroban/rpc"),
                Err(RpcUrlError::UnsupportedScheme("ws".into()))
            );
        }

        #[test]
        fn scheme_matching_is_case_insensitive() {
            assert!(validate_rpc_endpoint("HTTPS://rpc.example.test/rpc").is_ok());
            assert!(validate_rpc_endpoint("Http://localhost:8000/rpc").is_ok());
        }

        #[test]
        fn rejects_a_missing_host() {
            assert_eq!(
                validate_rpc_endpoint("https://"),
                Err(RpcUrlError::MissingHost)
            );
            assert_eq!(
                validate_rpc_endpoint("https:///soroban/rpc"),
                Err(RpcUrlError::MissingHost)
            );
        }

        #[test]
        fn rejects_malformed_authorities() {
            let cases = [
                ("https://rpc example.test/rpc", "rpc example.test"),
                ("https://[::1/soroban/rpc", "[::1"),
                ("https://:8000/soroban/rpc", ":8000"),
                ("https://::1/soroban/rpc", "::1"),
            ];
            for (url, authority) in cases {
                assert_eq!(
                    validate_rpc_endpoint(url),
                    Err(RpcUrlError::InvalidAuthority(authority.to_string())),
                    "{url}"
                );
            }
        }

        #[test]
        fn rejects_unusable_ports() {
            let cases = [
                ("https://rpc.example.test:/soroban/rpc", ""),
                ("https://rpc.example.test:notaport/soroban/rpc", "notaport"),
                ("https://rpc.example.test:0/soroban/rpc", "0"),
                ("https://rpc.example.test:65536/soroban/rpc", "65536"),
            ];
            for (url, port) in cases {
                assert_eq!(
                    validate_rpc_endpoint(url),
                    Err(RpcUrlError::InvalidPort(port.to_string())),
                    "{url}"
                );
            }
        }

        #[test]
        fn trims_surrounding_whitespace() {
            assert_eq!(
                validate_rpc_endpoint("  https://rpc.example.test/rpc  "),
                Ok("https://rpc.example.test/rpc")
            );
        }

        #[test]
        fn remote_plaintext_message_names_the_host_and_https() {
            let message = RpcUrlError::RemotePlaintext("rpc.example.test".into()).to_string();
            assert!(message.contains("rpc.example.test"), "{message}");
            assert!(message.contains("https://"), "{message}");
            assert!(message.contains("loopback"), "{message}");
        }
    }
}
