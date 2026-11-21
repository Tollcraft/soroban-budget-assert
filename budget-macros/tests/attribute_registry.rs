//! The impl-block skip list must name exactly the exported attributes (#993).
//!
//! `is_budget_attr` decides whether a method inside an annotated `impl` block is
//! left alone while the block is rewritten, and it reads the hand-maintained
//! `BUDGET_ATTR_NAMES` in `src/lib.rs`. A `#[proc_macro_attribute]` added
//! without a matching entry is invisible to that logic — the failure #829
//! repaired — and no other test notices, because the new attribute still
//! expands correctly on a bare `fn`. Only an annotated `impl` block tells the
//! two states apart.
//!
//! The list is a private `const`, so an integration test cannot read it as
//! data; it is parsed out of the source text instead. [`drift`] is pure over
//! that text, which is what lets the two failure directions be demonstrated
//! here without editing `src/lib.rs`.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// The line that decorates every macro exposed as an attribute.
const ATTR_MARKER: &str = "#[proc_macro_attribute]";

/// Anchor for the skip list, so the scan reads its items and not a later
/// mention of the same name in a comment.
const SKIP_LIST: &str = "const BUDGET_ATTR_NAMES";

fn source_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("lib.rs")
}

fn source() -> String {
    let path = source_path();
    match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) => panic!("cannot read {}: {err}", path.display()),
    }
}

/// The `pub fn` name the marker at `index` decorates.
///
/// Doc comments, plain comments and other attributes sit between the marker and
/// the signature; anything else means the marker does not decorate a function
/// this test understands, which is reported as `None` and caught by
/// [`every_declaration_is_named`] rather than skipped in silence.
fn signature_name(lines: &[&str], index: usize) -> Option<String> {
    for line in lines.get(index + 1..)? {
        let trimmed = line.trim_start();
        if trimmed.starts_with("///")
            || trimmed.starts_with("//")
            || trimmed.starts_with('#')
            || trimmed.is_empty()
        {
            continue;
        }
        return trimmed
            .strip_prefix("pub fn ")
            .and_then(|rest| rest.split('(').next())
            .map(|name| name.trim().to_string());
    }
    None
}

/// Every function `src/lib.rs` exports as an attribute macro.
fn exported(text: &str) -> BTreeSet<String> {
    let lines: Vec<&str> = text.lines().collect();
    lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.trim_end() == ATTR_MARKER)
        .filter_map(|(index, _)| signature_name(&lines, index))
        .collect()
}

/// The names written into `BUDGET_ATTR_NAMES`.
fn registered(text: &str) -> BTreeSet<String> {
    let declared = text
        .find(SKIP_LIST)
        .unwrap_or_else(|| panic!("no `{SKIP_LIST}` in the source"));
    let opened = declared
        + text[declared..]
            .find("= &[")
            .unwrap_or_else(|| panic!("`{SKIP_LIST}` is not a slice literal"));
    let closed = text[opened..]
        .find("];")
        .unwrap_or_else(|| panic!("`{SKIP_LIST}` never closes"));
    // The items are the odd fields of a split on `"`: the text before the first
    // quote, then name, separator, name, separator, …
    text[opened..opened + closed]
        .split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_string)
        .collect()
}

/// `(exported but unlisted, listed but not exported)`, each sorted.
fn drift(text: &str) -> (Vec<String>, Vec<String>) {
    let exported = exported(text);
    let registered = registered(text);
    (
        exported.difference(&registered).cloned().collect(),
        registered.difference(&exported).cloned().collect(),
    )
}

/// One attribute and a skip list that names it, so both failure directions can
/// be produced without touching the real source.
const ONE_ATTRIBUTE: &str = "\
#[proc_macro_attribute]
pub fn budget_example(attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

const BUDGET_ATTR_NAMES: &[&str] = &[
    \"budget_example\",
];
";

/// The list entry of [`ONE_ATTRIBUTE`], which is the only place its name appears
/// quoted.
const LISTED_ENTRY: &str = "    \"budget_example\",";

#[test]
fn the_skip_list_names_every_exported_attribute() {
    let (unlisted, stale) = drift(&source());
    assert!(
        unlisted.is_empty() && stale.is_empty(),
        "`BUDGET_ATTR_NAMES` and the `#[proc_macro_attribute]` functions disagree; \
         unlisted: {unlisted:?}, stale: {stale:?}. \
         An attribute missing from the list is not skipped inside an annotated \
         `impl` block, and a stale name means the list is no longer the set of \
         budget attributes."
    );
}

#[test]
fn every_declaration_is_named() {
    // The scan of the signatures has to account for every marker, or a marker
    // this test fails to parse would drop out of `exported` and the agreement
    // above would hold over a smaller set than the crate really exports.
    let text = source();
    let markers = text.matches(ATTR_MARKER).count();
    assert!(
        markers >= 8,
        "expected the {markers} `#[proc_macro_attribute]` declarations to still be there"
    );
    assert_eq!(
        exported(&text).len(),
        markers,
        "a `{ATTR_MARKER}` was not read as decorating a `pub fn`"
    );
}

#[test]
fn the_scan_reads_the_fixture_it_is_tested_against() {
    // Without this, an empty `exported` and an empty `registered` would agree
    // and every assertion below would pass over nothing.
    assert_eq!(
        exported(ONE_ATTRIBUTE),
        ["budget_example".to_string()].into_iter().collect()
    );
    assert_eq!(
        registered(ONE_ATTRIBUTE),
        ["budget_example".to_string()].into_iter().collect()
    );
    assert_eq!(drift(ONE_ATTRIBUTE), (Vec::new(), Vec::new()));
}

#[test]
fn a_new_attribute_left_out_of_the_list_is_reported() {
    // The #829 failure: the attribute exists, the list does not know it.
    let without = ONE_ATTRIBUTE.replace(LISTED_ENTRY, "");
    assert_eq!(
        drift(&without),
        (vec!["budget_example".to_string()], vec![])
    );
}

#[test]
fn a_list_entry_that_no_longer_exists_is_reported() {
    // The other direction: a rename or a removal leaves the list claiming an
    // attribute the crate stopped exporting.
    let renamed = ONE_ATTRIBUTE.replace(LISTED_ENTRY, "    \"budget_renamed\",");
    assert_eq!(
        drift(&renamed),
        (
            vec!["budget_example".to_string()],
            vec!["budget_renamed".to_string()]
        )
    );
}
