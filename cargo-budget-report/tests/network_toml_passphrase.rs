mod error {
    use std::fmt;

    #[derive(Debug)]
    pub enum Error {
        Message(String),
    }

    impl fmt::Display for Error {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::Message(message) => write!(f, "{message}"),
            }
        }
    }

    pub type Result<T> = std::result::Result<T, Error>;
}

#[path = "../src/network_guard.rs"]
mod network_guard;

use network_guard::{
    classify, ensure_deploy_allowed, resolve_passphrase, NetworkClass, TESTNET_PASSPHRASE,
};
use std::path::PathBuf;
use std::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn with_network_toml<T>(alias: &str, contents: &str, check: impl FnOnce(String) -> T) -> T {
    let _guard = ENV_LOCK.lock().unwrap();
    let root = tempfile::tempdir().expect("create config root");
    let network_dir = root.path().join("network");
    std::fs::create_dir_all(&network_dir).expect("create network dir");
    let path: PathBuf = network_dir.join(format!("{alias}.toml"));
    std::fs::write(&path, contents).expect("write network.toml");

    let previous = std::env::var_os("STELLAR_CONFIG_HOME");
    std::env::set_var("STELLAR_CONFIG_HOME", root.path());
    let resolved = resolve_passphrase(alias);
    let result = check(resolved);

    match previous {
        Some(value) => std::env::set_var("STELLAR_CONFIG_HOME", value),
        None => std::env::remove_var("STELLAR_CONFIG_HOME"),
    }
    result
}

#[test]
fn double_quoted_passphrase_with_padding_classifies_as_testnet() {
    with_network_toml(
        "quoted_double",
        "rpc_url = \"http://localhost:8000\"\n  network_passphrase   =   \"Test SDF Network ; September 2015\"  \n",
        |resolved| {
            assert_eq!(resolved, TESTNET_PASSPHRASE);
            assert_eq!(classify(&resolved), NetworkClass::Testnet);
            assert!(ensure_deploy_allowed("quoted_double", false).is_ok());
        },
    );
}

#[test]
fn single_quoted_passphrase_keeps_its_quotes_and_is_not_trusted() {
    with_network_toml(
        "quoted_single",
        "passphrase = 'Test SDF Network ; September 2015'\n",
        |resolved| {
            assert_eq!(resolved, "'Test SDF Network ; September 2015'");
            assert_eq!(classify(&resolved), NetworkClass::Unknown);
            assert!(ensure_deploy_allowed("quoted_single", false).is_err());
        },
    );
}

#[test]
fn an_inline_comment_is_not_stripped_from_the_passphrase() {
    with_network_toml(
        "with_comment",
        "network_passphrase = \"Test SDF Network ; September 2015\" # local note\n",
        |resolved| {
            assert!(
                resolved.contains("September 2015\" # local note"),
                "{resolved}"
            );
            assert_eq!(classify(&resolved), NetworkClass::Unknown);
        },
    );
}

#[test]
fn embedded_equals_signs_are_preserved_after_the_first_separator() {
    with_network_toml(
        "with_equals",
        "network_passphrase = \"Test = Network ; September 2015\"\n",
        |resolved| {
            assert_eq!(resolved, "Test = Network ; September 2015");
            assert!(resolved.contains(" = "));
            assert_eq!(classify(&resolved), NetworkClass::Unknown);
        },
    );
}

#[test]
fn the_first_non_empty_passphrase_line_wins() {
    with_network_toml(
        "first_non_empty",
        "passphrase = \"\"\npassphrase = \"Standalone Network ; February 2017\"\nnetwork_passphrase = \"Public Global Stellar Network ; September 2015\"\n",
        |resolved| {
            assert_eq!(resolved, "Standalone Network ; February 2017");
            assert_eq!(classify(&resolved), NetworkClass::Local);
        },
    );
}

#[test]
fn a_commented_out_key_does_not_resolve_the_alias() {
    with_network_toml(
        "commented_out",
        "# network_passphrase = \"Test SDF Network ; September 2015\"\n",
        |resolved| {
            assert_eq!(resolved, "commented_out");
            assert_eq!(classify(&resolved), NetworkClass::Unknown);
        },
    );
}
