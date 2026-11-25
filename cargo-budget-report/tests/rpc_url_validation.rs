//! Process-level coverage for issue #1061: a custom `--rpc-url` must be
//! validated before it reaches `curl` or the Stellar CLI.
//!
//! The rules (see `cargo-budget-report/src/url_checks.rs`) are:
//!
//! * `https://` for every non-loopback host, so the signed transaction
//!   envelope is never sent over an unencrypted connection;
//! * `http://` still allowed for loopback hosts (`localhost`, `127.0.0.1`,
//!   `[::1]`), because the traffic never leaves the machine;
//! * embedded userinfo (`user:password@host`) rejected, regardless of scheme;
//! * a well-formed `host[:port]` authority with a usable port.
//!
//! Each test runs the real compiled binary with the checked-in `fake_bin`
//! directory first on `PATH`, so the `stellar` / `curl` preflight passes and
//! no network call or funded Stellar identity is involved. Validation happens
//! before workspace discovery, so a rejected endpoint needs no contract
//! workspace at all.

use assert_cmd::Command;
use std::path::{Path, PathBuf};

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn fake_bin_dir() -> PathBuf {
    manifest_dir().join("tests/fixtures/fake_bin")
}

fn mocked_path() -> String {
    let real_path = std::env::var("PATH").unwrap_or_default();
    format!("{}:{}", fake_bin_dir().display(), real_path)
}

/// Passphrase that matches the `local` alias used in [`BASE_ARGS`].
const STANDALONE_PASSPHRASE: &str = "Standalone Network ; February 2017";

/// Arguments shared by every invocation. `--network local` keeps the
/// deploy-safety guard happy (it is a disposable network) and `--quiet`
/// leaves only the validation diagnostic on stderr.
const BASE_ARGS: [&str; 5] = ["budget-report", "--network", "local", "--source", "alice"];

fn rpc_cmd(dir: &Path, url: &str) -> Command {
    let mut cmd = Command::cargo_bin("cargo-budget-report").expect("binary should be built");
    cmd.current_dir(dir)
        .env("PATH", mocked_path())
        .args(BASE_ARGS)
        .args(["--quiet", "--rpc-url", url])
        .args(["--network-passphrase", STANDALONE_PASSPHRASE]);
    cmd
}

/// Asserts that `url` is refused with a `--rpc-url` diagnostic naming the
/// problem, before any workspace discovery or network traffic.
fn assert_rejected(url: &str, expected: &str) {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let assert = rpc_cmd(dir.path(), url).assert();
    let failed = assert.failure();
    let output = failed.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        stderr.contains("invalid --rpc-url"),
        "expected a --rpc-url diagnostic for {url:?}, got: {stderr}"
    );
    assert!(
        stderr.contains(expected),
        "expected {expected:?} in the diagnostic for {url:?}, got: {stderr}"
    );
    // A configuration-style mistake is a generic (non-retryable) failure.
    assert_eq!(
        output.status.code(),
        Some(1),
        "unexpected exit code for {url:?}: {stderr}"
    );
}

/// Asserts that `url` passes validation. The run continues past the endpoint
/// check, so the only thing under test here is that the URL was not rejected.
fn assert_accepted(url: &str) {
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let assert = rpc_cmd(dir.path(), url).assert();
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        !stderr.contains("invalid --rpc-url"),
        "endpoint {url:?} should have been accepted, got: {stderr}"
    );
}

// ── Rejections ──────────────────────────────────────────────────────────

#[test]
fn rejects_plaintext_remote_endpoint() {
    assert_rejected("http://rpc.example.test/soroban/rpc", "unencrypted");
}

#[test]
fn rejects_plaintext_with_an_explicit_port() {
    assert_rejected("http://rpc.example.test:8000/soroban/rpc", "unencrypted");
}

#[test]
fn rejects_embedded_userinfo_credentials() {
    assert_rejected(
        "https://user:secret@rpc.example.test/soroban/rpc",
        "embedded credentials",
    );
}

#[test]
fn rejects_missing_scheme() {
    assert_rejected("rpc.example.test/soroban/rpc", "missing URI scheme");
}

#[test]
fn rejects_unsupported_scheme() {
    assert_rejected(
        "ws://rpc.example.test/soroban/rpc",
        "unsupported scheme `ws`",
    );
}

#[test]
fn rejects_empty_url() {
    assert_rejected("", "the URL is empty");
}

#[test]
fn rejects_non_numeric_port() {
    assert_rejected(
        "https://rpc.example.test:notaport/soroban/rpc",
        "invalid port",
    );
}

#[test]
fn rejects_port_out_of_range() {
    assert_rejected("https://rpc.example.test:65536/soroban/rpc", "invalid port");
}

// ── Acceptances ─────────────────────────────────────────────────────────

#[test]
fn accepts_https_remote_endpoint() {
    assert_accepted("https://rpc.example.test/soroban/rpc");
}

#[test]
fn accepts_localhost_plaintext_endpoint() {
    assert_accepted("http://localhost:8000/soroban/rpc");
}

#[test]
fn accepts_loopback_ipv4_plaintext_endpoint() {
    assert_accepted("http://127.0.0.1:8000/soroban/rpc");
}

#[test]
fn accepts_loopback_ipv6_plaintext_endpoint() {
    assert_accepted("http://[::1]:8000/soroban/rpc");
}
