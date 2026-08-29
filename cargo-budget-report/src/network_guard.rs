//! Refuse to deploy against Stellar Mainnet unless the operator opts in.
//!
//! `cargo budget-report` deploys a throwaway contract and simulates calls
//! against it. On testnet that is free and disposable; pointed at Mainnet the
//! same run funds an account and pushes a real contract using real funds, and
//! nothing else in the pipeline treats Mainnet as different from any other
//! network. This module is the single gate: it classifies the *resolved*
//! network and stops the run before anything is built, funded, or deployed
//! when that network is Mainnet — or cannot be recognised — and
//! `--allow-mainnet` was not passed.
//!
//! The check is on the network passphrase, not the `--network` spelling: an
//! alias or an RPC URL can point anywhere, but the passphrase is the identity
//! the network actually signs with. A known alias (`testnet`, `mainnet`, …)
//! classifies directly; an unknown alias is resolved against the Stellar CLI's
//! own network config where possible, and treated as unsafe when it cannot be.

use crate::error::{Error, Result};
use std::path::PathBuf;

/// Passphrase of the Stellar public network. Deploying here spends real funds.
pub const MAINNET_PASSPHRASE: &str = "Public Global Stellar Network ; September 2015";
/// Passphrase of Stellar testnet — free and periodically reset.
pub const TESTNET_PASSPHRASE: &str = "Test SDF Network ; September 2015";
/// Passphrase of Futurenet — also a throwaway test network.
pub const FUTURENET_PASSPHRASE: &str = "Test SDF Future Network ; October 2022";
/// Passphrase of a local standalone network (quickstart container).
pub const STANDALONE_PASSPHRASE: &str = "Standalone Network ; February 2017";

/// What a resolved `--network` value amounts to for deploy-safety purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkClass {
    /// Stellar testnet — free, disposable, the tool's default target.
    Testnet,
    /// Futurenet — also a throwaway test network.
    Futurenet,
    /// A local / standalone network.
    Local,
    /// Stellar public network. Deploying here spends real funds.
    Mainnet,
    /// Anything not recognised. Treated as unsafe: defaulting to permissive
    /// here is the exact failure mode this module exists to prevent.
    Unknown,
}

impl NetworkClass {
    /// True when a run may build and deploy against this network without an
    /// explicit `--allow-mainnet` opt-in.
    pub fn is_disposable(self) -> bool {
        matches!(self, Self::Testnet | Self::Futurenet | Self::Local)
    }

    /// Human-readable name for the refusal message.
    fn describe(self) -> &'static str {
        match self {
            Self::Testnet => "testnet",
            Self::Futurenet => "futurenet",
            Self::Local => "a local / standalone network",
            Self::Mainnet => "Stellar Mainnet (the public network)",
            Self::Unknown => "an unrecognised network",
        }
    }
}

/// Classify a network passphrase or a well-known alias.
///
/// Passphrases (which always contain `" ; "`) are matched exactly; a
/// passphrase-shaped string that matches nothing known is [`NetworkClass::Unknown`]
/// rather than being re-interpreted as an alias. Aliases are matched
/// case-insensitively.
pub fn classify(network: &str) -> NetworkClass {
    let trimmed = network.trim();

    match trimmed {
        MAINNET_PASSPHRASE => return NetworkClass::Mainnet,
        TESTNET_PASSPHRASE => return NetworkClass::Testnet,
        FUTURENET_PASSPHRASE => return NetworkClass::Futurenet,
        STANDALONE_PASSPHRASE => return NetworkClass::Local,
        _ => {}
    }

    if trimmed.contains(" ; ") {
        // Looks like a passphrase but is not one we know. Do not fall
        // through to matching its words against the alias list.
        return NetworkClass::Unknown;
    }

    match trimmed.to_ascii_lowercase().as_str() {
        "testnet" | "test" => NetworkClass::Testnet,
        "futurenet" | "future" => NetworkClass::Futurenet,
        "local" | "localnet" | "standalone" => NetworkClass::Local,
        "mainnet" | "pubnet" | "publicnet" | "public" => NetworkClass::Mainnet,
        _ => NetworkClass::Unknown,
    }
}

/// Best-effort resolution of a `--network` alias to its passphrase using the
/// Stellar CLI's own on-disk network config.
///
/// Returns the passphrase when `network` names a config file under
/// `<stellar-config>/network/<name>.toml`; otherwise returns `network`
/// unchanged. This is what makes the guard resistant to a custom alias that
/// points a familiar-looking name at Mainnet's RPC — the passphrase in that
/// file is what gets classified, not the alias.
pub fn resolve_passphrase(network: &str) -> String {
    resolve_passphrase_in(&stellar_config_dirs(), network)
}

/// [`resolve_passphrase`] against an explicit set of config roots, so the
/// lookup can be exercised without touching process-global environment.
fn resolve_passphrase_in(dirs: &[PathBuf], network: &str) -> String {
    let name = network.trim();
    if name.is_empty() || name.contains(" ; ") {
        return network.to_string();
    }
    for dir in dirs {
        let candidate = dir.join("network").join(format!("{name}.toml"));
        let Ok(contents) = std::fs::read_to_string(&candidate) else {
            continue;
        };
        if let Some(passphrase) = passphrase_from_network_toml(&contents) {
            return passphrase;
        }
    }
    network.to_string()
}

/// Candidate Stellar CLI config roots, most specific first.
fn stellar_config_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(explicit) = std::env::var_os("STELLAR_CONFIG_HOME") {
        let base = PathBuf::from(explicit);
        dirs.push(base.join(".config").join("stellar"));
        dirs.push(base.join("stellar"));
        dirs.push(base);
    }
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        dirs.push(PathBuf::from(xdg).join("stellar"));
    }
    if let Some(home) = std::env::var_os("HOME") {
        dirs.push(PathBuf::from(home).join(".config").join("stellar"));
    }
    dirs
}

/// Pull `network_passphrase = "..."` (or `passphrase = "..."`) out of a
/// Stellar CLI network config file without a full TOML parse — the file
/// shape is stable and a dependency-free scan keeps this cheap.
fn passphrase_from_network_toml(contents: &str) -> Option<String> {
    for line in contents.lines() {
        let line = line.trim();
        let Some(after_key) = line
            .strip_prefix("network_passphrase")
            .or_else(|| line.strip_prefix("passphrase"))
        else {
            continue;
        };
        let Some(after_eq) = after_key.trim_start().strip_prefix('=') else {
            continue;
        };
        let unquoted = after_eq.trim().trim_matches('"');
        if !unquoted.is_empty() {
            return Some(unquoted.to_string());
        }
    }
    None
}

/// Stop the run unless it is safe to deploy against `network`.
///
/// Called once, before any package is built, funded, or deployed:
///
/// * a disposable network (testnet / futurenet / local) always passes;
/// * Mainnet passes only with `allow_mainnet`;
/// * an unrecognised network passes only with `allow_mainnet` — the
///   conservative default is to refuse rather than risk a real deploy.
///
/// The refusal message states which network was detected and how to proceed.
pub fn ensure_deploy_allowed(network: &str, allow_mainnet: bool) -> Result<()> {
    let resolved = resolve_passphrase(network);
    let class = classify(&resolved);

    if class.is_disposable() {
        return Ok(());
    }
    if allow_mainnet {
        return Ok(());
    }

    Err(Error::Message(format!(
        "refusing to run against {desc}: the resolved network for --network / budget.toml \
         value {value:?} is not a disposable test network.\n\
         `cargo budget-report` deploys a contract and simulates calls against it; against \
         Mainnet that funds a source account and pushes a contract using real funds.\n\
         Re-run with --allow-mainnet if you deliberately mean to target this network.",
        desc = class.describe(),
        value = network.trim(),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_known_aliases_case_insensitively() {
        assert_eq!(classify("testnet"), NetworkClass::Testnet);
        assert_eq!(classify("TestNet"), NetworkClass::Testnet);
        assert_eq!(classify(" futurenet "), NetworkClass::Futurenet);
        assert_eq!(classify("local"), NetworkClass::Local);
        assert_eq!(classify("mainnet"), NetworkClass::Mainnet);
        assert_eq!(classify("PUBLIC"), NetworkClass::Mainnet);
    }

    #[test]
    fn classifies_known_passphrases() {
        assert_eq!(classify(MAINNET_PASSPHRASE), NetworkClass::Mainnet);
        assert_eq!(classify(TESTNET_PASSPHRASE), NetworkClass::Testnet);
        assert_eq!(classify(FUTURENET_PASSPHRASE), NetworkClass::Futurenet);
        assert_eq!(classify(STANDALONE_PASSPHRASE), NetworkClass::Local);
    }

    #[test]
    fn unknown_alias_and_unknown_passphrase_are_unknown() {
        assert_eq!(classify("my-custom-net"), NetworkClass::Unknown);
        assert_eq!(
            classify("Weird Private Network ; January 2024"),
            NetworkClass::Unknown
        );
    }

    #[test]
    fn passphrase_shaped_string_does_not_fall_through_to_alias_words() {
        // Contains the word "mainnet" but is passphrase-shaped and unknown.
        assert_eq!(
            classify("totally not mainnet ; today"),
            NetworkClass::Unknown
        );
    }

    #[test]
    fn disposable_networks_pass_without_opt_in() {
        for net in ["testnet", "futurenet", "local", TESTNET_PASSPHRASE] {
            assert!(
                ensure_deploy_allowed(net, false).is_ok(),
                "{net} should pass"
            );
        }
    }

    #[test]
    fn mainnet_is_refused_without_opt_in() {
        let err = ensure_deploy_allowed("mainnet", false)
            .unwrap_err()
            .to_string();
        assert!(err.contains("Mainnet"), "message names the network: {err}");
        assert!(
            err.contains("--allow-mainnet"),
            "message says how to proceed: {err}"
        );
    }

    #[test]
    fn mainnet_is_allowed_with_opt_in() {
        assert!(ensure_deploy_allowed(MAINNET_PASSPHRASE, true).is_ok());
    }

    #[test]
    fn unknown_network_is_refused_without_opt_in() {
        let err = ensure_deploy_allowed("some-private-net", false)
            .unwrap_err()
            .to_string();
        assert!(err.contains("unrecognised network"), "got: {err}");
    }

    #[test]
    fn unknown_network_is_allowed_with_opt_in() {
        assert!(ensure_deploy_allowed("some-private-net", true).is_ok());
    }

    #[test]
    fn passphrase_extraction_reads_either_key() {
        assert_eq!(
            passphrase_from_network_toml(
                "network_passphrase = \"Public Global Stellar Network ; September 2015\"\n"
            )
            .as_deref(),
            Some(MAINNET_PASSPHRASE)
        );
        assert_eq!(
            passphrase_from_network_toml("rpc_url = \"http://localhost:8000\"\npassphrase=\"Standalone Network ; February 2017\"").as_deref(),
            Some(STANDALONE_PASSPHRASE)
        );
        assert_eq!(passphrase_from_network_toml("rpc_url = \"x\"\n"), None);
    }

    #[test]
    fn resolve_passphrase_follows_a_custom_alias_to_mainnet() {
        let tmp = std::env::temp_dir().join(format!(
            "budget_report_netguard_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let root = tmp.join("stellar");
        std::fs::create_dir_all(root.join("network")).unwrap();
        std::fs::write(
            root.join("network").join("looks-safe.toml"),
            "rpc_url = \"https://mainnet.example\"\nnetwork_passphrase = \"Public Global Stellar Network ; September 2015\"\n",
        )
        .unwrap();

        let resolved = resolve_passphrase_in(&[root], "looks-safe");
        let _ = std::fs::remove_dir_all(&tmp);

        assert_eq!(resolved, MAINNET_PASSPHRASE);
        assert_eq!(classify(&resolved), NetworkClass::Mainnet);
    }
}
