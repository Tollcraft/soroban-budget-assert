//! Issue #978 pins, from the test side, the same contract
//! `.github/scripts/check-compatibility-docs.sh` enforces on the shell side:
//! the README's compatibility section must carry **two** records — the protocol
//! the manifests pin and the protocol observed on the public networks — and they
//! must stay two records rather than collapsing into one sentence that could
//! mean either.
//!
//! Every check here reads files under the repository root and opens no socket.
//! A gate that called an RPC endpoint would be a flaky gate; the live numbers
//! are recorded as dated observations, and re-deriving them is a manual command
//! the README has to keep written down.

use std::fs;
use std::path::{Path, PathBuf};

/// Header that opens the README section this suite owns.
const SECTION_START: &str = "## ⚙️ Supported Versions & Compatibility";

/// Header that closes it: the records live between the two.
const SECTION_END: &str = "### What \"Supported\" means here";

/// Label of the record derived from the manifests.
const PINNED_LABEL: &str = "- **Pinned protocol**:";

/// Label of the record read off a running network.
const OBSERVED_LABEL: &str = "- **Observed live protocol**:";

/// The three manifests the compatibility section documents.
const MANIFESTS: [&str; 3] = [
    "cargo-budget-report/Cargo.toml",
    "amm-pool-contract/Cargo.toml",
    "host-function-contract/Cargo.toml",
];

/// The dependency keys whose versions the README has to carry.
const PINNED_DEPENDENCIES: [&str; 2] = ["soroban-sdk", "stellar-xdr"];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cargo-budget-report lives one level below the repo root")
        .to_path_buf()
}

fn read(path: &Path) -> String {
    match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => panic!("cannot read {}: {error}", path.display()),
    }
}

fn readme() -> String {
    read(&repo_root().join("README.md"))
}

/// The README's compatibility section, sliced the way
/// `.github/scripts/check-compatibility-docs.sh` slices it.
fn compatibility_section() -> String {
    let text = readme();
    let start = text
        .find(SECTION_START)
        .unwrap_or_else(|| panic!("README has no {SECTION_START:?} section"));
    let end = text[start..]
        .find(SECTION_END)
        .unwrap_or_else(|| panic!("README section is not closed by {SECTION_END:?}"));
    text[start..start + end].to_string()
}

/// The single line that starts with `label`, panicking with the label's name.
fn bullet<'a>(section: &'a str, label: &str) -> &'a str {
    let mut matches = section
        .lines()
        .map(str::trim_end)
        .filter(|line| line.starts_with(label));
    let line = matches
        .next()
        .unwrap_or_else(|| panic!("README compatibility section lost the {label:?} record"));
    assert!(
        matches.next().is_none(),
        "README has more than one line starting with {label:?}"
    );
    line
}

/// The number in the first `Protocol <n>` token of `text`, if any.
fn protocol_token(text: &str) -> Option<u64> {
    let after = text
        .find("Protocol ")
        .map(|at| &text[at + "Protocol ".len()..])?;
    digits(after)
}

/// The number written directly after `network`, as in "mainnet 29".
fn number_after(text: &str, network: &str) -> Option<u64> {
    let after = text
        .find(network)
        .map(|at| text[at + network.len()..].trim_start())?;
    digits(after)
}

/// The leading run of ASCII digits in `text`, if it has one.
fn digits(text: &str) -> Option<u64> {
    let run: String = text.chars().take_while(char::is_ascii_digit).collect();
    if run.is_empty() {
        return None;
    }
    run.parse().ok()
}

/// `true` when `text` contains an ISO `YYYY-MM-DD` date.
fn has_iso_date(text: &str) -> bool {
    text.as_bytes().windows(10).any(|window| {
        let digit = |index: usize| window[index].is_ascii_digit();
        digit(0)
            && digit(1)
            && digit(2)
            && digit(3)
            && window[4] == b'-'
            && digit(5)
            && digit(6)
            && window[7] == b'-'
            && digit(8)
            && digit(9)
    })
}

/// Every exact `"x.y.z"` pinned for `dependency` in `manifest`, mirroring the
/// `^(soroban-sdk|stellar-xdr) =` anchor the shell gate uses.
fn pins_in(manifest: &str, dependency: &str) -> Vec<String> {
    let text = read(&repo_root().join(manifest));
    text.lines()
        .map(str::trim)
        .filter(|line| {
            line.strip_prefix(dependency)
                .is_some_and(|rest| rest.trim_start().starts_with('='))
        })
        .filter_map(|line| line.split('"').nth(1).map(str::to_string))
        .filter(|version| version.chars().next().is_some_and(|ch| ch.is_ascii_digit()))
        .collect()
}

/// The `stellar-xdr` major version the CLI crate requires.
fn xdr_major() -> u64 {
    let pins = pins_in("cargo-budget-report/Cargo.toml", "stellar-xdr");
    let pin = pins
        .first()
        .unwrap_or_else(|| panic!("cargo-budget-report/Cargo.toml declares no stellar-xdr pin"));
    digits(pin).unwrap_or_else(|| panic!("unparseable stellar-xdr pin {pin:?}"))
}

#[test]
fn the_pinned_protocol_is_recorded_and_matches_the_manifest() {
    let section = compatibility_section();
    let pinned = bullet(&section, PINNED_LABEL);
    let stated = protocol_token(pinned)
        .unwrap_or_else(|| panic!("{PINNED_LABEL} must state `Protocol <n>`: {pinned}"));
    let major = xdr_major();
    assert_eq!(
        stated, major,
        "the README pins Protocol {stated} but `stellar-xdr` is pinned to {major}.x.x"
    );
}

#[test]
fn the_pinned_record_carries_no_live_observation() {
    let pinned = bullet(&compatibility_section(), PINNED_LABEL).to_lowercase();
    for merged in ["mainnet", "testnet", "observed"] {
        assert!(
            !pinned.contains(merged),
            "the {PINNED_LABEL} record describes the manifests only, but names \
             {merged:?}: the live observation has been merged back into it"
        );
    }
}

#[test]
fn the_observed_record_names_both_networks_and_the_date() {
    let section = compatibility_section();
    let observed = bullet(&section, OBSERVED_LABEL);
    for network in ["mainnet", "testnet"] {
        assert!(
            number_after(observed, network).is_some(),
            "{OBSERVED_LABEL} must give a protocol number for {network}: {observed}"
        );
    }
    assert!(
        has_iso_date(observed),
        "{OBSERVED_LABEL} must date the observation as YYYY-MM-DD: {observed}"
    );
}

#[test]
fn the_observed_record_does_not_restate_the_pinned_protocol() {
    let section = compatibility_section();
    let observed = bullet(&section, OBSERVED_LABEL);
    assert!(
        protocol_token(observed).is_none(),
        "only the {PINNED_LABEL} record may carry a `Protocol <n>` token, so the \
         two can never be collapsed into one sentence: {observed}"
    );
}

#[test]
fn the_observation_command_is_written_down() {
    let section = compatibility_section();
    // The invocation, not the word: a sentence that merely mentions the RPC
    // method would leave the number unre-derivable.
    for fragment in ["\"method\":\"getNetwork\"", "horizon.stellar.org"] {
        assert!(
            section.contains(fragment),
            "the README must record how to re-check the observed number, but \
             {fragment:?} is gone from the compatibility section"
        );
    }
}

#[test]
fn the_untested_rows_survive_the_split() {
    let untested = readme().matches("**Untested**").count();
    assert!(
        untested >= 2,
        "the compatibility matrix must keep its **Untested** rows; found {untested}"
    );
}

#[test]
fn every_manifest_pin_is_still_documented() {
    let readme = readme();
    let mut checked = 0;
    for manifest in MANIFESTS {
        for dependency in PINNED_DEPENDENCIES {
            for pin in pins_in(manifest, dependency) {
                checked += 1;
                assert!(
                    readme.contains(&pin),
                    "{manifest} pins {dependency} {pin} but README.md no longer documents it"
                );
            }
        }
    }
    assert!(
        checked > 0,
        "the pin scan found nothing to check - the manifest reader broke"
    );
}
