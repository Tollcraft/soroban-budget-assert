//! Issue #990: the trybuild matrix of macro attributes against limit forms.
//!
//! `tests/ui.rs` hands trybuild two globs (`tests/ui/*.rs` and
//! `tests/ui/pass/*.rs`) and pins the contents of the fixtures it names, but
//! nothing recorded *which* cells the suite is supposed to have. A fixture that
//! disappears takes its coverage with it silently. The tables below are that
//! record: one row per attribute per limit form, each naming the pass fixtures
//! that must exist for it, checked against the directory both ways - a listed
//! file that goes missing fails naming its cell, and a new fixture that no cell
//! claims fails the same way.
//!
//! Two deliberate limits on the shape of the record:
//!
//! * `tests/ui.rs` already protects compile-fail fixtures (every one must ship a
//!   non-empty `.stderr`), and its set is an order of magnitude larger than the
//!   behavioural one, so this table tracks the *pass* fixtures: those are the
//!   cells that actually execute the expansion.
//! * The four limit forms are the `BudgetLimit` source keys (`env`, `env_file`,
//!   `config`, `pct`) at `budget-macros/src/lib.rs:265`. The integer-literal form
//!   every fixture uses by default is not a cell, and `baseline = <expr>` is not
//!   a limit form but an orthogonal axis, so it gets its own table.
//!
//! The fixture lists are derived from disk by `fixtures_for`, the same way the
//! issue asks ("derive the list from `rust_fixtures()`"), and compared with the
//! declared const tables. `ui_dir` / `rust_fixtures` are duplicated from
//! `tests/ui.rs:23-39` because integration tests are separate crates and those
//! helpers are private there; `tests/ui.rs` is not this issue's file to change.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// The eight attribute macros `budget-macros` exports.
const ATTRIBUTES: [&str; 8] = [
    "budget_cpu_lt",
    "budget_write_bytes_lt",
    "budget_read_bytes_lt",
    "budget_events_lt",
    "budget_ledger_entries_lt",
    "budget_mem_lt",
    "budget_lt",
    "budget_scaling",
];

/// The four `BudgetLimit` source keys.
const LIMIT_FORMS: [&str; 4] = ["env", "env_file", "config", "pct"];

/// `baseline = <expr>` is subtracted from the measured cost before the
/// comparison, on any attribute that takes a limit; it is tracked separately
/// because it is an argument of the attribute, not a source for the limit.
const BASELINE_KEY: &str = "baseline";

/// One cell of the matrix: the pass fixtures that must expand `attribute` with
/// a limit read through `form`.
#[derive(Clone, Copy)]
struct Cell {
    attribute: &'static str,
    form: &'static str,
    fixtures: &'static [&'static str],
}

impl Cell {
    /// The name a failure uses, so a missing fixture is traceable to its cell.
    fn label(&self) -> String {
        format!("{} × {}", self.attribute, self.form)
    }
}

/// `attribute × form` for the four limit forms. Cells with an empty list are
/// recorded gaps: nothing expands that attribute with that limit source yet.
///
/// The fixture lists are named consts rather than inline `[..]` slices because
/// the const evaluator on this toolchain rejects the unsizing range-index
/// inside a `const` block.
const NONE: &[&str] = &[];
const CPU_ENV: &[&str] = &[
    "pass_env_file.rs",
    "pass_env_file_duplicate_key.rs",
    "pass_env_file_hash_suffix.rs",
    "pass_env_file_malformed.rs",
    "pass_env_var_limit.rs",
    "pass_percentage_limit.rs",
];
const CPU_ENV_FILE: &[&str] = &[
    "pass_env_file.rs",
    "pass_env_file_duplicate_key.rs",
    "pass_env_file_hash_suffix.rs",
    "pass_env_file_malformed.rs",
    "pass_percentage_limit.rs",
];
const CPU_CONFIG: &[&str] = &["pass_config_limit.rs"];
const CPU_PCT: &[&str] = &["pass_percentage_limit.rs"];
const EVENTS_ENV: &[&str] = &["pass_events.rs", "pass_events_env_file.rs"];
const EVENTS_PCT: &[&str] = &["pass_events.rs"];
const LT_LIMITED: &[&str] = &[
    "pass_budget_lt.rs",
    "pass_budget_lt_mixed_sources.rs",
    "pass_nested_pct.rs",
];
const LT_CONFIG: &[&str] = &["pass_budget_lt.rs"];
const READ_BYTES_ENV: &[&str] = &["pass_read_bytes_env_file_baseline.rs"];
const READ_BYTES_ENV_FILE: &[&str] = &["pass_read_bytes_env_file_baseline.rs"];

const MATRIX: [Cell; 32] = {
    // `budget_scaling` takes `sizes`/`model`/`tolerance` and no limit source at
    // all, so all four of its cells are structural gaps.
    let forms = [
        [CPU_ENV, CPU_ENV_FILE, CPU_CONFIG, CPU_PCT],
        [NONE, NONE, NONE, NONE],
        [READ_BYTES_ENV, READ_BYTES_ENV_FILE, NONE, NONE],
        [EVENTS_ENV, EVENTS_ENV, NONE, EVENTS_PCT],
        [NONE, NONE, NONE, NONE],
        [NONE, NONE, NONE, NONE],
        [LT_LIMITED, LT_LIMITED, LT_CONFIG, LT_LIMITED],
        [NONE, NONE, NONE, NONE],
    ];
    let mut cells = [Cell {
        attribute: "",
        form: "",
        fixtures: NONE,
    }; 32];
    let mut attribute_index = 0;
    while attribute_index < ATTRIBUTES.len() {
        let mut form_index = 0;
        while form_index < LIMIT_FORMS.len() {
            cells[attribute_index * LIMIT_FORMS.len() + form_index] = Cell {
                attribute: ATTRIBUTES[attribute_index],
                form: LIMIT_FORMS[form_index],
                fixtures: forms[attribute_index][form_index],
            };
            form_index += 1;
        }
        attribute_index += 1;
    }
    cells
};

/// Which pass fixtures subtract a baseline per attribute.
///
/// `budget_lt` is absent by design: its combined form takes `cpu_baseline` and
/// `mem_baseline` rather than a single `baseline`, which is a different argument
/// and not this axis.
const BASELINE_CELLS: [(&str, &[&str]); 8] = [
    (
        "budget_cpu_lt",
        &[
            "pass_baseline.rs",
            "pass_env_ident_standalone.rs",
            "pass_impl_block.rs",
        ],
    ),
    ("budget_write_bytes_lt", &["pass_baseline.rs"]),
    (
        "budget_read_bytes_lt",
        &[
            "pass_read_bytes_baseline.rs",
            "pass_read_bytes_env_file_baseline.rs",
        ],
    ),
    (
        "budget_events_lt",
        &["pass_events.rs", "pass_events_ledger_impl_block.rs"],
    ),
    (
        "budget_ledger_entries_lt",
        &[
            "pass_ledger_baseline.rs",
            "pass_ledger_breakdown.rs",
            "pass_events_ledger_impl_block.rs",
        ],
    ),
    ("budget_mem_lt", &["pass_baseline.rs"]),
    ("budget_lt", &[]),
    ("budget_scaling", &[]),
];

/// Root of the trybuild fixtures, anchored at the crate so the tests run from
/// any working directory. Duplicated from `tests/ui.rs:23`.
fn ui_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("ui")
}

/// `.rs` files directly inside `dir`, sorted, mirroring trybuild's non-recursive
/// globs. Duplicated from `tests/ui.rs:31`.
fn rust_fixtures(dir: &Path) -> Vec<PathBuf> {
    let entries = fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("cannot read fixture directory {}: {error}", dir.display()));
    let mut fixtures: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| matches!(path.extension(), Some(ext) if ext == "rs"))
        .collect();
    fixtures.sort();
    fixtures
}

fn read(fixture: &Path) -> String {
    match fs::read_to_string(fixture) {
        Ok(text) => text,
        Err(error) => panic!("cannot read {}: {error}", fixture.display()),
    }
}

fn file_name(fixture: &Path) -> String {
    fixture
        .file_name()
        .expect("a fixture path ends in a file name")
        .to_string_lossy()
        .into_owned()
}

fn pass_fixtures() -> Vec<PathBuf> {
    rust_fixtures(&ui_dir().join("pass"))
}

/// `source` with comments removed, so prose that quotes an attribute argument
/// cannot invent coverage. String contents are kept.
fn code_only(source: &str) -> String {
    let chars: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut index = 0;
    let mut in_string = false;
    let mut block_depth: usize = 0;
    while index < chars.len() {
        let ch = chars[index];
        let next = if index + 1 < chars.len() {
            chars[index + 1]
        } else {
            '\0'
        };
        if in_string {
            out.push(ch);
            if ch == '\\' && index + 1 < chars.len() {
                out.push(next);
                index += 2;
                continue;
            }
            if ch == '"' {
                in_string = false;
            }
            index += 1;
            continue;
        }
        if block_depth > 0 {
            if ch == '*' && next == '/' {
                block_depth -= 1;
                index += 2;
                continue;
            }
            if ch == '/' && next == '*' {
                block_depth += 1;
                index += 2;
                continue;
            }
            if ch == '\n' {
                out.push('\n');
            }
            index += 1;
            continue;
        }
        if ch == '/' && next == '/' {
            while index < chars.len() && chars[index] != '\n' {
                index += 1;
            }
            continue;
        }
        if ch == '/' && next == '*' {
            block_depth += 1;
            index += 2;
            continue;
        }
        if ch == '"' {
            in_string = true;
        }
        out.push(ch);
        index += 1;
    }
    out
}

/// The `(attribute, argument text)` pairs of every `#[budget_* …]` invocation in
/// `source`. Brackets are counted over comment-stripped text, which is enough
/// for these fixtures: none of them carries a bracket inside a string literal.
fn attribute_invocations(source: &str) -> Vec<(String, String)> {
    let code = code_only(source);
    let chars: Vec<char> = code.chars().collect();
    let mut invocations = Vec::new();
    let mut index = 0;
    while index + 1 < chars.len() {
        if chars[index] != '#' || chars[index + 1] != '[' {
            index += 1;
            continue;
        }
        let mut name_end = index + 2;
        while name_end < chars.len()
            && (chars[name_end].is_ascii_alphanumeric() || chars[name_end] == '_')
        {
            name_end += 1;
        }
        let name: String = chars[index + 2..name_end].iter().collect();
        if !name.starts_with("budget_") || name_end >= chars.len() || chars[name_end] != '(' {
            index += 1;
            continue;
        }
        let mut depth = 0usize;
        let mut cursor = name_end;
        let args_start = cursor + 1;
        while cursor < chars.len() {
            match chars[cursor] {
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
            cursor += 1;
        }
        if depth != 0 {
            break;
        }
        invocations.push((name, chars[args_start..cursor].iter().collect()));
        index = cursor + 1;
    }
    invocations
}

/// True when `arguments` assigns `key` - at any nesting depth, because a limit
/// form can sit inside `of = …` - meaning the key is followed by `=` and not
/// preceded by identifier characters.
fn uses_key(arguments: &str, key: &str) -> bool {
    let bytes = arguments.as_bytes();
    let is_ident = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_';
    let mut from = 0;
    while let Some(offset) = arguments[from..].find(key) {
        let start = from + offset;
        let end = start + key.len();
        let whole_word = start == 0 || !is_ident(bytes[start - 1]);
        let mut probe = end;
        while probe < bytes.len() && matches!(bytes[probe], b' ' | b'\t' | b'\n' | b'\r') {
            probe += 1;
        }
        let assigns =
            probe < bytes.len() && bytes[probe] == b'=' && bytes.get(probe + 1) != Some(&b'=');
        if whole_word && assigns {
            return true;
        }
        from = end;
    }
    false
}

/// The pass fixtures that expand `attribute` with `key` among its arguments.
fn fixtures_for(attribute: &str, key: &str) -> BTreeSet<String> {
    pass_fixtures()
        .into_iter()
        .filter(|fixture| {
            attribute_invocations(&read(fixture))
                .into_iter()
                .any(|(name, arguments)| name == attribute && uses_key(&arguments, key))
        })
        .map(|fixture| file_name(&fixture))
        .collect()
}

fn declared(fixtures: &[&str]) -> BTreeSet<String> {
    fixtures.iter().map(|name| (*name).to_string()).collect()
}

/// The matrix is only a record if it enumerates the whole cross product.
#[test]
fn matrix_has_every_attribute_and_limit_form_exactly_once() {
    assert_eq!(MATRIX.len(), ATTRIBUTES.len() * LIMIT_FORMS.len());
    let mut seen = BTreeSet::new();
    for cell in MATRIX.iter() {
        assert!(
            ATTRIBUTES.contains(&cell.attribute),
            "matrix names an attribute that is not exported: {}",
            cell.attribute
        );
        assert!(
            LIMIT_FORMS.contains(&cell.form),
            "matrix names a limit form that is not a BudgetLimit source: {}",
            cell.form
        );
        assert!(
            seen.insert((cell.attribute, cell.form)),
            "the {} × {} cell is declared twice",
            cell.attribute,
            cell.form
        );
    }
    for attribute in ATTRIBUTES {
        for form in LIMIT_FORMS {
            assert!(
                seen.contains(&(attribute, form)),
                "the matrix lost the {attribute} × {form} cell"
            );
        }
    }
    assert_eq!(BASELINE_CELLS.len(), ATTRIBUTES.len());
}

/// A listed fixture that vanishes fails here, naming the cell it belonged to.
#[test]
fn every_fixture_named_by_the_matrix_exists() {
    let pass_dir = ui_dir().join("pass");
    for cell in MATRIX.iter() {
        for fixture in cell.fixtures {
            assert!(
                pass_dir.join(fixture).is_file(),
                "{}: pass fixture {fixture} is missing",
                cell.label()
            );
        }
    }
    for &(attribute, fixtures) in BASELINE_CELLS.iter() {
        for fixture in fixtures {
            assert!(
                pass_dir.join(fixture).is_file(),
                "{attribute} × {BASELINE_KEY}: pass fixture {fixture} is missing"
            );
        }
    }
}

/// Each declared fixture must still expand *that* attribute with *that* form:
/// renaming or repurposing a file is a matrix change, not a quiet one.
#[test]
fn every_named_fixture_still_expands_its_own_cell() {
    let pass_dir = ui_dir().join("pass");
    for cell in MATRIX.iter() {
        for fixture in cell.fixtures {
            let path = pass_dir.join(fixture);
            if !path.is_file() {
                continue; // reported by every_fixture_named_by_the_matrix_exists
            }
            assert!(
                fixtures_for(cell.attribute, cell.form).contains(&file_name(&path)),
                "{}: {fixture} no longer expands {} with a {form} limit",
                cell.label(),
                fixture,
                form = cell.form
            );
        }
    }
    for &(attribute, fixtures) in BASELINE_CELLS.iter() {
        for fixture in fixtures {
            let path = pass_dir.join(fixture);
            if !path.is_file() {
                continue;
            }
            assert!(
                fixtures_for(attribute, BASELINE_KEY).contains(&file_name(&path)),
                "{attribute} × {BASELINE_KEY}: {fixture} no longer passes a baseline"
            );
        }
    }
}

/// And the other direction: a fixture that covers a cell no longer claims it is
/// an unrecorded cell, which is what this table exists to prevent.
#[test]
fn no_cell_gains_or_loses_a_fixture_without_updating_the_matrix() {
    // Every drifted cell at once: whoever adds a fixture gets the whole list to
    // paste into the table, instead of one cell per compile.
    let mut drift: Vec<String> = Vec::new();
    for cell in MATRIX.iter() {
        let (derived, claimed) = (
            fixtures_for(cell.attribute, cell.form),
            declared(cell.fixtures),
        );
        if derived != claimed {
            drift.push(format!(
                "{}: on disk {derived:?}, matrix claims {claimed:?}",
                cell.label()
            ));
        }
    }
    for &(attribute, fixtures) in BASELINE_CELLS.iter() {
        let (derived, claimed) = (fixtures_for(attribute, BASELINE_KEY), declared(fixtures));
        if derived != claimed {
            drift.push(format!(
                "{attribute} × {BASELINE_KEY}: on disk {derived:?}, matrix claims {claimed:?}"
            ));
        }
    }
    assert!(
        drift.is_empty(),
        "{} cell(s) drifted from the table, so MATRIX/BASELINE_CELLS must be updated:\n{}",
        drift.len(),
        drift.join("\n")
    );
}

/// Issue #990 named two open cells. They stay filled, and each new fixture has
/// to be the behavioural kind: it must run, not merely compile.
#[test]
fn the_two_open_cells_issue_990_named_are_filled() {
    let pass_dir = ui_dir().join("pass");
    for (cell, fixture, needles) in [
        (
            "budget_events_lt × env_file",
            "pass_events_env_file.rs",
            ["budget_events_lt", "env_file", "budget_panic", "fn main"],
        ),
        (
            "budget_ledger_entries_lt × baseline",
            "pass_ledger_baseline.rs",
            [
                "budget_ledger_entries_lt",
                "baseline",
                "budget_panic",
                "fn main",
            ],
        ),
    ] {
        let path = pass_dir.join(fixture);
        assert!(path.is_file(), "{cell}: {fixture} is missing");
        let source = read(&path);
        for needle in needles {
            assert!(
                source.contains(needle),
                "{cell}: {fixture} must contain {needle:?} so the macro is expanded into code that runs"
            );
        }
    }
    // `pass_events_env_file.rs` reads its limit straight from the env file rather
    // than as the reference of a percentage, which is the distinction the cell
    // records; `fixtures_for` alone cannot tell the two apart.
    let events_env_file = read(&pass_dir.join("pass_events_env_file.rs"));
    assert!(
        events_env_file.contains("env_file = ENV_FILE, env = "),
        "budget_events_lt × env_file: the fixture must use env_file as the limit source"
    );
}

/// The globs trybuild is given still match both fixture directories, so a
/// restructure of `tests/ui/` cannot leave this matrix comparing nothing.
#[test]
fn both_fixture_directories_are_present() {
    let compile_fail = rust_fixtures(&ui_dir());
    let pass = pass_fixtures();
    assert!(
        !compile_fail.is_empty() && !pass.is_empty(),
        "tests/ui needs both compile-fail fixtures ({} found) and pass fixtures ({} found)",
        compile_fail.len(),
        pass.len()
    );
    // Every pass fixture is a standalone program, which is what makes the
    // matrix's cells executable rather than merely compilable.
    for fixture in &pass {
        assert!(
            read(fixture).contains("fn main"),
            "{}: needs main()",
            fixture.display()
        );
    }
}
