//! Budget assertion procedural macros for Soroban contract tests.
//!
//! # `std`/`no_std` boundary
//!
//! This crate is a **proc-macro library** — it runs at compile time and has no
//! runtime footprint. It is used **exclusively in `#[cfg(test)]` contexts**:
//! `amm-pool-contract/tests/` and the crate's own UI tests.
//!
//! The macros emit code that references `std::env::var`, `std::fs::read_to_string`,
//! and `std::path::Path`. This is correct because the generated code is compiled
//! into test binaries where `std` is always available. The macros are **not**
//! intended for use in `no_std` Soroban contracts.
//!
//! If future use cases require `no_std` macro expansion, the `EnvFile` and
//! `Config` limit forms would need an alternative to `std::fs` (e.g., a
//! `#[cfg(not(no_std))]` gate or a compile-time-only resolution path). The
//! integer-literal and `env` forms would work unchanged in `no_std` contexts.

extern crate proc_macro;

use std::path::{Path, PathBuf};
#[cfg(test)]
mod parser_props;

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{quote, ToTokens};
use syn::visit_mut::{self, VisitMut};
use syn::{
    parse::Parse, parse::ParseStream, parse_quote, Attribute, Expr, Ident, ItemFn, LitFloat,
    LitInt, LitStr, Token,
};

#[derive(Clone)]
enum BudgetLimit {
    Int(u64),
    EnvVar(String),
    Config(String),
    /// Read from a `KEY=VALUE` file at `path`, looking up `var_name`.
    ///
    /// The file format is the standard `.env` shape: one `KEY=VALUE` per
    /// line, comments (`#`) and blank lines ignored. Reads happen at test
    /// runtime, so a single checked-in `tier-a-limits.env` can drive many
    /// tests without any global environment mutation (and therefore no
    /// `unsafe std::env::set_var` call).
    EnvFile {
        path: proc_macro2::TokenStream,
        var_name: String,
    },
    /// A limit expressed as a percentage of a reference limit.
    ///
    /// `pct` is the percentage (1–100). `of` is the source for the
    /// reference limit (typically `env_file` + `env` pointing at a
    /// network-wide limit in `tier-a-limits.env`).
    Percentage {
        pct: u64,
        of: Box<BudgetLimit>,
    },
}

#[derive(Clone, Default)]
struct BudgetSpec {
    cpu: Option<BudgetLimit>,
    mem: Option<BudgetLimit>,
    cpu_baseline: Option<Expr>,
    mem_baseline: Option<Expr>,
    env_ident: Option<Ident>,
}

/// A limit plus an optional baseline, for the single-metric attributes
/// (`budget_cpu_lt`, `budget_mem_lt`, `budget_write_bytes_lt`, `budget_read_bytes_lt`).
///
/// `budget_lt` takes its baselines as the separate `cpu_baseline` /
/// `mem_baseline` keys instead, because it carries two metrics at once.
#[derive(Clone)]
struct StandaloneSpec {
    limit: BudgetLimit,
    baseline: Option<Expr>,
}

impl Parse for StandaloneSpec {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let limit: BudgetLimit = input.parse()?;

        if input.is_empty() {
            return Ok(StandaloneSpec {
                limit,
                baseline: None,
            });
        }

        input.parse::<Token![,]>()?;
        let ident: Ident = input.parse()?;
        if ident != "baseline" {
            return Err(syn::Error::new(
                ident.span(),
                format!("expected `baseline`, got `{ident}`"),
            ));
        }
        input.parse::<Token![=]>()?;
        let baseline: Expr = input.parse()?;

        if !input.is_empty() {
            return Err(syn::Error::new(
                input.span(),
                format!(
                    "unexpected token(s) after `baseline = …` — expected end of attribute, got `{}`",
                    input
                ),
            ));
        }

        Ok(StandaloneSpec {
            limit,
            baseline: Some(baseline),
        })
    }
}

/// Resolves a literal `env_file` path at macro-expansion time, mirroring the
/// candidate roots the generated runtime lookup walks.
///
/// Returns the first existing file, or `None` if the path resolves nowhere —
/// which the caller turns into a compile error. The bases are, in order: the
/// path as given (absolute or relative to the compiler's CWD), then
/// `CARGO_MANIFEST_DIR` (the crate being compiled), each also probed through a
/// `budget-macros/` / `../budget-macros/` prefix so the workspace-relative
/// paths the runtime resolver accepts do not become build errors here.
fn resolve_env_file_at_expansion(path: &str) -> Option<PathBuf> {
    let raw = Path::new(path);
    if raw.is_file() {
        return Some(raw.to_path_buf());
    }

    let mut bases: Vec<PathBuf> = Vec::new();
    if let Ok(dir) = std::env::var("CARGO_MANIFEST_DIR") {
        bases.push(PathBuf::from(dir));
    }
    if let Ok(cwd) = std::env::current_dir() {
        bases.push(cwd);
    }

    for base in bases {
        let direct = base.join(raw);
        if direct.is_file() {
            return Some(direct);
        }
        for prefix in ["budget-macros", "../budget-macros"] {
            let prefixed = base.join(prefix).join(raw);
            if prefixed.is_file() {
                return Some(prefixed);
            }
        }
    }

    None
}

/// True when the next tokens (optionally after a leading comma) name one of
/// the `BudgetLimit` *source* keys.
///
/// Used to tell "this literal is being illegally combined with a second source
/// for the same value" apart from "this literal is complete and what follows
/// belongs to the caller" (a sibling `mem = …`, or `, baseline = …`).
fn peeks_limit_source_key(input: ParseStream) -> bool {
    let ahead = input.fork();
    if ahead.peek(Token![,]) {
        let _ = ahead.parse::<Token![,]>();
    }
    ahead
        .parse::<Ident>()
        .map(|i| {
            matches!(
                i.to_string().as_str(),
                "env" | "env_file" | "config" | "pct"
            )
        })
        .unwrap_or(false)
}

/// Parses the attribute arguments for `budget_cpu_lt` / `budget_mem_lt`
/// into a concrete [`BudgetLimit`] value.
///
/// Accepted forms:
/// - An integer literal (e.g. `950_000`).
/// - `env = "VAR_NAME"` to read the limit from a process environment
///   variable at test time.
/// - `config = "key"` to read the limit from `budget.json` in the
///   process working directory.
/// - `env_file = "PATH"` paired with `env = "VAR_NAME"`: read the limit
///   from a `KEY=VALUE` (`.env`-shaped) file at `PATH`. The file is read
///   at test runtime on each invocation; failures surface as test panics
///   naming the file, key, and parse failure.
///
/// All four forms can be combined with the existing per-spec knobs on
/// `#[budget_lt(...)]` (e.g. `cpu = env_file = "...", env = "..."`).
impl Parse for BudgetLimit {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut env_var: Option<String> = None;
        let mut env_file: Option<proc_macro2::TokenStream> = None;
        let mut config_key: Option<String> = None;
        let mut pct_value: Option<u64> = None;
        let mut pct_of: Option<Box<BudgetLimit>> = None;
        let mut pct_span: Option<Span> = None;
        // Spans of the key tokens, for precise error pointers.
        let mut env_span: Option<Span> = None;
        let mut env_file_span: Option<Span> = None;
        let mut config_span: Option<Span> = None;
        // First identifier seen that is not a limit-source key. Only used to
        // improve the error when nothing valid was parsed at all.
        let mut unknown_key: Option<Ident> = None;

        // The leading form may also be a bare integer literal. Detect that
        // case before parsing identifiers.
        if input.peek(LitInt) {
            let lit: LitInt = input.parse()?;
            // A literal names the limit outright, so pairing it with `env` /
            // `env_file` / `config` — which name a *source* for that same
            // value — is contradictory and stays rejected. Anything else that
            // follows belongs to the caller and is left in the stream: a
            // sibling `mem = …` when parsed from `BudgetSpec`, or the
            // `, baseline = …` that `StandaloneSpec` consumes.
            if peeks_limit_source_key(input) {
                return Err(syn::Error::new(
                    lit.span(),
                    "integer literal cannot be combined with env / config / env_file / pct",
                ));
            }
            return Ok(BudgetLimit::Int(lit.base10_parse()?));
        }

        while !input.is_empty() {
            // When called from BudgetSpec::parse the input may contain
            // tokens for other spec keys (`cpu`, `mem`, `env_ident`).
            // Only consume key=value pairs whose key is a known
            // BudgetLimit key; stop before anything else.
            // `fork()`, not `clone()`: `ParseStream` is `&ParseBuffer`, so
            // cloning the reference aliases the same buffer and the
            // "lookahead" parse below would consume from the real stream.
            if input.peek(Token![,]) {
                let ahead = input.fork();
                let _ = ahead.parse::<Token![,]>();
                if !(ahead.peek(Ident)
                    && matches!(
                        ahead.fork().parse::<Ident>().unwrap().to_string().as_str(),
                        "env" | "env_file" | "config" | "pct"
                    ))
                {
                    break;
                }
                input.parse::<Token![,]>()?;
            } else if input.peek(Ident) {
                let ahead = input.fork();
                let key: Ident = ahead.parse().unwrap();
                if !matches!(
                    key.to_string().as_str(),
                    "env" | "env_file" | "config" | "pct"
                ) {
                    // Remember it: if no limit source turns up at all, this is
                    // the token the user got wrong, and naming it beats a
                    // generic "expected one of …" pointed at the whole
                    // attribute.
                    unknown_key = Some(key);
                    break;
                }
            } else {
                break;
            }

            let ident: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let ident_str = ident.to_string();
            if ident_str == "env_file" {
                env_file_span = Some(ident.span());
                // Accept either a string literal or an identifier/const path
                // for env_file, so callers can write `env_file = "path"` or
                // `env_file = CONST_NAME`.
                let path: proc_macro2::TokenStream = if input.peek(LitStr) {
                    let lit: LitStr = input.parse()?;
                    // A literal path is knowable now, so a typo or a file that
                    // was never checked in should fail the build here — not at
                    // test runtime, and never silently. A non-literal path
                    // (`env_file = CONST` / an expression) may be produced by
                    // the build, so it stays a runtime resolution.
                    if resolve_env_file_at_expansion(&lit.value()).is_none() {
                        return Err(syn::Error::new(
                            lit.span(),
                            format!(
                                "env_file {:?} was not found at macro-expansion time \
                                 (looked relative to CARGO_MANIFEST_DIR and the build's \
                                 working directory). Create the file, fix the path, or \
                                 pass it as a `const` if it is generated during the build.",
                                lit.value()
                            ),
                        ));
                    }
                    lit.into_token_stream()
                } else {
                    let expr: Expr = input.parse()?;
                    expr.into_token_stream()
                };
                env_file = Some(path);
            } else if ident_str == "pct" {
                // Parse: `pct = <number>` followed by optional `, of = <source>`.
                // The `of` source is parsed as a nested BudgetLimit.
                pct_span = Some(ident.span());
                if pct_value.is_some() {
                    return Err(syn::Error::new(
                        ident.span(),
                        "`pct` cannot be specified more than once",
                    ));
                }
                let pct_lit: LitInt = input.parse()?;
                let pct: u64 = pct_lit.base10_parse()?;
                if !(1..=100).contains(&pct) {
                    return Err(syn::Error::new(
                        pct_lit.span(),
                        format!("percentage must be between 1 and 100, got {pct}"),
                    ));
                }
                pct_value = Some(pct);

                // Parse optional `, of = <source>` where <source> is itself a
                // BudgetLimit (typically `env_file = "..."` + `env = "..."`).
                if input.peek(Token![,]) {
                    let ahead = input.fork();
                    let _ = ahead.parse::<Token![,]>();
                    if ahead.peek(Ident) {
                        let ahead_key: Ident = ahead.parse().unwrap();
                        if ahead_key == "of" {
                            input.parse::<Token![,]>()?;
                            input.parse::<Ident>()?; // consume `of`
                            input.parse::<Token![=]>()?;
                            let of_limit: BudgetLimit = input.parse()?;
                            pct_of = Some(Box::new(of_limit));
                        }
                    }
                }

                // Reject trailing `env` or `config` keys — they provide an
                // absolute value and are meaningless alongside `pct`.
                if input.peek(Token![,]) {
                    let ahead = input.fork();
                    let _ = ahead.parse::<Token![,]>();
                    if ahead.peek(Ident) {
                        let ahead_key: Ident = ahead.parse().unwrap();
                        let key_name = ahead_key.to_string();
                        if matches!(key_name.as_str(), "env" | "config") {
                            return Err(syn::Error::new(
                                ahead_key.span(),
                                format!(
                                    "`pct` cannot be combined with `{}` \
                             — use `pct = N, of = env_file = \"...\", env = \"...\"` instead",
                                    key_name
                                ),
                            ));
                        }
                    }
                }

                // After parsing pct (+ optional of), stop: remaining tokens
                // belong to the caller (a sibling `baseline = …`, or the
                // enclosing spec).
                break;
            } else {
                let lit: LitStr = input.parse()?;
                match ident_str.as_str() {
                    "env" => {
                        env_span = Some(ident.span());
                        env_var = Some(lit.value())
                    }
                    "config" => {
                        config_span = Some(ident.span());
                        config_key = Some(lit.value())
                    }
                    other => {
                        return Err(syn::Error::new(
                            ident.span(),
                            format!("expected `env`, `env_file`, or `config`, got `{other}`"),
                        ));
                    }
                }
            }
        }

        // Combine the collected parts into the right `BudgetLimit` variant.
        // Precedence: `pct` → `Percentage`; `env_file` + `env` → `EnvFile`;
        // `env` only → `EnvVar`; `config` → `Config`. Mixing types is
        // rejected to avoid silent confusion.
        if let Some(pct) = pct_value {
            // `pct` requires `of = <source>` to know which reference limit
            // to take the percentage of.
            let of = pct_of.ok_or_else(|| {
                syn::Error::new(
                    pct_span.unwrap_or_else(Span::call_site),
                    format!(
                        "`pct` requires `of = <source>` — e.g. \
                         `pct = {pct}, of = env_file = \"tier-a-limits.env\", env = \"NETWORK__CPU\"`"
                    ),
                )
            })?;
            return Ok(BudgetLimit::Percentage { pct, of });
        }
        match (env_file, env_var, config_key) {
            (Some(path), Some(var), None) => Ok(BudgetLimit::EnvFile {
                path,
                var_name: var,
            }),
            (None, Some(var), None) => Ok(BudgetLimit::EnvVar(var)),
            (None, None, Some(key)) => Ok(BudgetLimit::Config(key)),
            (Some(_), None, None) => Err(syn::Error::new(
                env_file_span.unwrap_or_else(Span::call_site),
                "`env_file` must be paired with `env = \"VAR_NAME\"`",
            )),
            (Some(_), _, Some(_)) => Err(syn::Error::new(
                env_file_span.unwrap_or_else(Span::call_site),
                "`env_file` cannot be paired with `config` — use `env_file = \"PATH\", env = \"VAR_NAME\"` instead",
            )),
            (None, Some(_), Some(_)) => Err(syn::Error::new(
                config_span
                    .or(env_span)
                    .unwrap_or_else(Span::call_site),
                "`env` and `config` cannot be combined — pick one",
            )),
            (None, None, None) => Err(match unknown_key {
                Some(key) => syn::Error::new(
                    key.span(),
                    format!(
                        "expected `env`, `env_file`, `config`, or `pct`, got `{key}`"
                    ),
                ),
                None => syn::Error::new(
                    proc_macro2::Span::call_site(),
                    "expected an integer literal, `env = \"VAR\"`, `env_file = \"PATH\"` + `env = \"VAR\"`, `config = \"KEY\"`, or `pct = N, of = <source>`",
                ),
            }),
        }
    }
}

impl Parse for BudgetSpec {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut spec = BudgetSpec::default();

        while !input.is_empty() {
            let ident: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let ident_str = ident.to_string();

            if ident_str == "env_ident" {
                spec.env_ident = Some(input.parse()?);
            } else if ident_str == "cpu" {
                spec.cpu = Some(input.parse()?);
            } else if ident_str == "mem" {
                spec.mem = Some(input.parse()?);
            } else if ident_str == "cpu_baseline" {
                spec.cpu_baseline = Some(input.parse()?);
            } else if ident_str == "mem_baseline" {
                spec.mem_baseline = Some(input.parse()?);
            } else {
                return Err(syn::Error::new(
                    ident.span(),
                    format!("unknown property: {}", ident_str),
                ));
            }

            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }

        if spec.cpu.is_none() && spec.mem.is_none() {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "must provide at least one of `cpu` or `mem` limits",
            ));
        }

        // A baseline is subtracted from its own metric's measurement, so one
        // without the matching limit silently does nothing. Reject it rather
        // than let the assertion quietly not exist.
        if spec.cpu_baseline.is_some() && spec.cpu.is_none() {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "`cpu_baseline` requires a `cpu` limit",
            ));
        }
        if spec.mem_baseline.is_some() && spec.mem.is_none() {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "`mem_baseline` requires a `mem` limit",
            ));
        }

        Ok(spec)
    }
}

fn generate_limit_expr(limit: &BudgetLimit, metric_label: &str) -> proc_macro2::TokenStream {
    match limit {
        BudgetLimit::Int(n) => quote! { #n },
        BudgetLimit::EnvVar(var) => quote! {
            budget_env_resolve(#var)
                .map(|s| s.parse::<u64>().unwrap_or_else(|_| {
                    panic!(
                        "{}: env var {}={:?} is not a valid u64",
                        #metric_label,
                        #var,
                        s
                    )
                }))
                .unwrap_or(u64::MAX)
        },
        BudgetLimit::Config(key) => quote! {
            std::fs::read_to_string(std::path::Path::new("budget.json"))
                .ok()
                .map(|content| {
                    parse_config_value(&content, #key).unwrap_or_else(|| {
                        panic!(
                            "{}: key '{}' not found or invalid in budget.json",
                            #metric_label,
                            #key,
                        )
                    })
                })
                .unwrap_or(u64::MAX)
        },
        BudgetLimit::EnvFile { path, var_name } => {
            // The closure is generated inside the test body, so each
            // assertion reads the file fresh (no shared mutable state).
            // File-not-found / missing-key / parse failures panic with a
            // caller-actionable message that names the file, key, and
            // offending value.
            quote! {
                {
                    let env_file_path: &str = #path;
                    let env_file_key: &str = #var_name;
                    let mut content_opt = std::fs::read_to_string(env_file_path).ok();
                    if content_opt.is_none() {
                        let candidates = [
                            format!("budget-macros/{}", env_file_path),
                            format!("../budget-macros/{}", env_file_path),
                            format!("../../budget-macros/{}", env_file_path),
                            format!("../../../budget-macros/{}", env_file_path),
                            format!("../../../../budget-macros/{}", env_file_path),
                        ];
                        for c in &candidates {
                            if let Ok(c_content) = std::fs::read_to_string(c) {
                                content_opt = Some(c_content);
                                break;
                            }
                        }
                    }
                    let resolved = content_opt.and_then(|content| {
                        parse_env_file_value(&content, env_file_key)
                    });
                    resolved.map(|s| {
                        s.trim().parse::<u64>().unwrap_or_else(|_| {
                            panic!(
                                "{}: env_file {} key {}={:?} is not a valid u64",
                                #metric_label,
                                env_file_path,
                                env_file_key,
                                s
                            )
                        })
                    }).unwrap_or_else(|| {
                        // Missing key in env_file — panic with an actionable
                        // message that names both the file and the key so a
                        // contributor who broke the wiring can see both at
                        // once.
                        panic!(
                            "{}: env_file '{}' is missing key '{}' — \
                             add it to the file or use a fallback limit",
                            #metric_label,
                            env_file_path,
                            env_file_key,
                        );
                    })
                }
            }
        }
        BudgetLimit::Percentage { pct, of } => {
            // The `of` source must be an EnvFile — the only form that reads
            // a named key from a checked-in file. Generate code that reads
            // the reference limit, computes `reference × pct / 100`, and
            // returns it.
            let ref_expr = match of.as_ref() {
                BudgetLimit::EnvFile { path, var_name } => quote! {
                    {
                        let __pct_env_path: &str = #path;
                        let __pct_env_key: &str = #var_name;
                        let mut __pct_content_opt = std::fs::read_to_string(__pct_env_path).ok();
                        if __pct_content_opt.is_none() {
                            let __pct_candidates = [
                                format!("budget-macros/{}", __pct_env_path),
                                format!("../budget-macros/{}", __pct_env_path),
                                format!("../../budget-macros/{}", __pct_env_path),
                                format!("../../../budget-macros/{}", __pct_env_path),
                                format!("../../../../budget-macros/{}", __pct_env_path),
                            ];
                            for __c in &__pct_candidates {
                                if let Ok(__c_content) = std::fs::read_to_string(__c) {
                                    __pct_content_opt = Some(__c_content);
                                    break;
                                }
                            }
                        }
                        __pct_content_opt
                            .and_then(|__c| parse_env_file_value(&__c, __pct_env_key))
                            .map(|__s| {
                                __s.trim().parse::<u64>().unwrap_or_else(|_| {
                                    panic!(
                                        "{}: percentage_of env_file {} key {}={:?} is not a valid u64",
                                        #metric_label,
                                        __pct_env_path,
                                        __pct_env_key,
                                        __s
                                    )
                                })
                            })
                            .unwrap_or_else(|| {
                                panic!(
                                    "{}: percentage_of env_file {} missing key {} (or file cannot be read)",
                                    #metric_label,
                                    __pct_env_path,
                                    __pct_env_key,
                                )
                            })
                    }
                },
                _ => unreachable!("the parser ensures `of` is an env_file when used with `pct`"),
            };
            quote! {
                {
                    let __pct_ref_limit: u64 = #ref_expr;
                    __pct_ref_limit * #pct / 100
                }
            }
        }
    }
}

/// Attribute paths that carry their own budget assertion. A method inside a
/// budget-annotated `impl` block that already wears one of these is left for
/// that attribute to expand — the block-level attribute skips it.
const BUDGET_ATTR_NAMES: &[&str] = &[
    "budget_cpu_lt",
    "budget_mem_lt",
    "budget_write_bytes_lt",
    "budget_read_bytes_lt",
    "budget_lt",
    "budget_scaling",
];

fn is_budget_attr(attr: &Attribute) -> bool {
    attr.path()
        .segments
        .last()
        .map(|s| BUDGET_ATTR_NAMES.contains(&s.ident.to_string().as_str()))
        .unwrap_or(false)
}

/// Builds the `(plain, marginal)` assertion-failure format strings for one
/// metric.
///
/// `fn_label` is empty for a bare `#[test] fn`, keeping the message
/// byte-identical to before. On a method reached through a budget-annotated
/// `impl` block it is the method name, inserted as `` [fn `name`] `` so a
/// failing block names the specific offender.
fn assert_messages(
    metric_phrase: &str,
    tail: &str,
    limit: &BudgetLimit,
    fn_label: &str,
) -> (String, String) {
    let ctx = if fn_label.is_empty() {
        String::new()
    } else {
        format!(" [fn `{fn_label}`]")
    };

    let pct_str = match limit {
        BudgetLimit::Percentage { pct, .. } => format!(" ({pct}% of network limit)"),
        _ => String::new(),
    };
    let marginal_pct_str = match limit {
        BudgetLimit::Percentage { pct, .. } => format!(", {pct}% of network limit"),
        _ => String::new(),
    };

    (
        format!("{metric_phrase} {{}} exceeded limit {{}}{pct_str}{ctx} - {tail}"),
        format!(
            "{metric_phrase} {{}} exceeded limit {{}}{ctx} \
             (marginal: {{}} measured - {{}} baseline{marginal_pct_str}) - {tail}"
        ),
    )
}

/// Applies a per-function budget expansion to either a bare `fn` or every
/// method of an `impl` block.
///
/// - Bare `fn`: `expand(input_fn, "")` — unchanged behaviour.
/// - `impl` block: every `fn` in the block that does not already carry its own
///   `#[budget_*]` attribute is instrumented (`expand(method_as_fn, name)`);
///   a method with its own budget attribute is left for that attribute to
///   expand, so a per-method limit overrides the block-level one. Helper and
///   non-`pub` methods are instrumented too — "every function in it" is taken
///   literally; exclude one with its own no-op attribute if that is not wanted.
/// - Anything else: the original `fn`-parse error, so `#[budget_*] struct …`
///   still fails with "expected `fn`".
fn expand_targets(item: TokenStream, expand: impl Fn(ItemFn, &str) -> TokenStream) -> TokenStream {
    let tokens: proc_macro2::TokenStream = item.clone().into();

    let fn_err = match syn::parse::<ItemFn>(item) {
        Ok(input_fn) => return expand(input_fn, ""),
        Err(e) => e,
    };

    let mut item_impl = match syn::parse2::<syn::ItemImpl>(tokens) {
        Ok(block) => block,
        Err(_) => return TokenStream::from(fn_err.to_compile_error()),
    };

    let mut instrumented = 0usize;
    for impl_item in &mut item_impl.items {
        let syn::ImplItem::Fn(method) = impl_item else {
            continue;
        };
        if method.attrs.iter().any(is_budget_attr) {
            continue;
        }

        let as_fn = ItemFn {
            attrs: method.attrs.clone(),
            vis: method.vis.clone(),
            modifiers: method.modifiers.clone(),
            sig: method.sig.clone(),
            block: Box::new(method.block.clone()),
        };
        let label = method.sig.ident.to_string();
        let expanded: proc_macro2::TokenStream = expand(as_fn, &label).into();
        match syn::parse2::<syn::ImplItemFn>(expanded) {
            Ok(new_method) => {
                *method = new_method;
                instrumented += 1;
            }
            Err(e) => return TokenStream::from(e.to_compile_error()),
        }
    }

    if instrumented == 0 {
        return TokenStream::from(
            syn::Error::new_spanned(
                &item_impl,
                "#[budget_*] on this impl block instrumented no methods: it has no \
                 functions, or every function already carries its own budget attribute",
            )
            .to_compile_error(),
        );
    }

    TokenStream::from(quote! { #item_impl })
}

/// Builds one metric's measurement + assertion.
///
/// Without a baseline this is the historical behaviour: measure, compare to
/// the limit. With one, the baseline is subtracted from the measurement first
/// and the *marginal* cost is what gets compared.
///
/// The subtraction saturates. A measurement below the baseline means the
/// baseline probe was the more expensive of the two — noise around a
/// near-zero marginal cost — and clamping to 0 reports that honestly as "no
/// measurable marginal cost" instead of wrapping to a huge u64 and failing.
///
/// The raw measurement is taken *before* the baseline expression is evaluated,
/// so a baseline helper that spins up its own `Env` cannot perturb the number
/// being asserted on.
///
/// `plain_msg` is used when there is no baseline; `marginal_msg` when there
/// is. Both expect format placeholders `{}` in the order:
/// - Plain: `{cost} {limit}`
/// - Marginal: `{marginal} {limit} {cost} {baseline}`
fn generate_metric_assert(
    cost_ident: &proc_macro2::Ident,
    cost_expr: proc_macro2::TokenStream,
    limit_expr: &proc_macro2::TokenStream,
    baseline: Option<&Expr>,
    plain_msg: &str,
    marginal_msg: &str,
) -> proc_macro2::TokenStream {
    match baseline {
        None => quote! {
            let #cost_ident = #cost_expr;
            let limit_u64: u64 = #limit_expr;
            assert!(
                #cost_ident < limit_u64,
                #plain_msg,
                #cost_ident,
                limit_u64
            );
        },
        Some(baseline_expr) => quote! {
            let #cost_ident = #cost_expr;
            let __budget_baseline: u64 = #baseline_expr;
            let __budget_marginal: u64 = #cost_ident.saturating_sub(__budget_baseline);
            let limit_u64: u64 = #limit_expr;
            assert!(
                __budget_marginal < limit_u64,
                #marginal_msg,
                __budget_marginal,
                limit_u64,
                #cost_ident,
                __budget_baseline
            );
        },
    }
}

/// Builds the ledger-entry measurement + assertion for `#[budget_ledger_entries_lt]`.
///
/// `Budget` does not expose a combined "entries" getter, so the read and write
/// entry counts are read separately from the cost tracker (`DiskReadEntries`
/// and `DiskWriteEntries`) and summed. The total is what the network enforces as
/// its single combined entry limit, but the failure message always reports the
/// read/write breakdown so a breach is never ambiguous about which side blew the
/// budget.
///
/// `ContractCostType` is referenced unqualified, so it must be in scope at the
/// call site (e.g. `use soroban_sdk::ContractCostType;`).
fn generate_ledger_assert(
    cost_ident: &proc_macro2::Ident,
    env_ident: &proc_macro2::Ident,
    limit_expr: &proc_macro2::TokenStream,
    baseline: Option<&Expr>,
) -> proc_macro2::TokenStream {
    let read_entries = quote! {
        #env_ident.cost_estimate().budget().tracker(ContractCostType::DiskReadEntries).iterations()
    };
    let write_entries = quote! {
        #env_ident.cost_estimate().budget().tracker(ContractCostType::DiskWriteEntries).iterations()
    };
    match baseline {
        None => quote! {
            let __read_entries = #read_entries;
            let __write_entries = #write_entries;
            let #cost_ident = __read_entries.saturating_add(__write_entries);
            let limit_u64: u64 = #limit_expr;
            assert!(
                #cost_ident < limit_u64,
                "Ledger entry count (read: {}, write: {}, total: {}) exceeded limit {} \
                 - local estimate, real network entry counts may differ",
                __read_entries,
                __write_entries,
                #cost_ident,
                limit_u64
            );
        },
        Some(baseline_expr) => quote! {
            let __read_entries = #read_entries;
            let __write_entries = #write_entries;
            let #cost_ident = __read_entries.saturating_add(__write_entries);
            let __budget_baseline: u64 = #baseline_expr;
            let __budget_marginal: u64 = #cost_ident.saturating_sub(__budget_baseline);
            let limit_u64: u64 = #limit_expr;
            assert!(
                __budget_marginal < limit_u64,
                "Ledger entry count (read: {}, write: {}, total: {}) exceeded limit {} \
                 (marginal: {} measured - {} baseline) \
                 - local estimate, real network entry counts may differ",
                __read_entries,
                __write_entries,
                #cost_ident,
                limit_u64,
                __budget_marginal,
                __budget_baseline
            );
        },
    }
}

/// Splits a test body into its leading statements and an optional trailing
/// expression.
///
/// A test may end in a tail expression (`Ok(())` for a `Result`-returning
/// test). Appending the budget assertion after that tail would not parse, so
/// the tail is captured into a binding, the assertion runs, and the captured
/// value is returned unchanged.
const MACRO_RETURN_ERROR: &str = "`return` inside a macro invocation is not supported by the \
     budget macros: the assertion cannot be injected into macro tokens, so it would be skipped \
     silently. Move the `return` out of the macro invocation, or end the test with a tail \
     expression instead. (If this `return` belongs to a closure passed to the macro, lift the \
     closure into a `let` binding before the macro call.)";

/// Rewrites the test body so the budget assertion runs on every path that leaves
/// the test function.
///
/// `return e` becomes `return { let v = e; <assertion>; v }`, which keeps the
/// expression's `!` type (so `return` in value position still type-checks) and
/// evaluates `e` before measuring, so the returned value's own cost is counted.
///
/// `return`s belonging to a nested closure, `async` block, or nested item exit
/// that inner body rather than the test, so those are left untouched. `return`s
/// hidden inside macro invocation tokens cannot be rewritten and are collected
/// so the caller can report them.
struct ReturnRewriter {
    assertion: proc_macro2::TokenStream,
    macro_returns: Vec<Span>,
}

impl VisitMut for ReturnRewriter {
    fn visit_expr_mut(&mut self, expr: &mut Expr) {
        // A `return` in here leaves the closure/async body, not the test function.
        if matches!(expr, Expr::Closure(_) | Expr::Async(_)) {
            return;
        }

        // Rewrite inner expressions first so the injected assertion is not revisited.
        visit_mut::visit_expr_mut(self, expr);

        if let Expr::Return(ret) = expr {
            let assertion = &self.assertion;
            *expr = match ret.expr.take() {
                Some(value) => parse_quote! {
                    return {
                        let __budget_returned = #value;
                        #assertion
                        __budget_returned
                    }
                },
                None => parse_quote! {
                    return {
                        #assertion
                    }
                },
            };
        }
    }

    fn visit_item_mut(&mut self, _item: &mut syn::Item) {
        // Nested items (e.g. helper `fn`s declared in the body) have their own returns.
    }

    fn visit_macro_mut(&mut self, mac: &mut syn::Macro) {
        if let Some(span) = find_return_token(mac.tokens.clone()) {
            self.macro_returns.push(span);
        }
    }
}

/// Finds a `return` token anywhere in a macro's token stream.
fn find_return_token(tokens: proc_macro2::TokenStream) -> Option<Span> {
    for token in tokens {
        match token {
            proc_macro2::TokenTree::Ident(ident) if ident == "return" => return Some(ident.span()),
            proc_macro2::TokenTree::Group(group) => {
                if let Some(span) = find_return_token(group.stream()) {
                    return Some(span);
                }
            }
            _ => {}
        }
    }
    None
}

/// Rebuilds `input_fn`'s body as `prelude`, the original statements, and
/// `assertion` on every path that leaves the function.
///
/// A trailing expression is bound before the assertion and yielded afterwards, so
/// it stays the function's value and `-> Result<_, _>` bodies compile; early
/// `return`s carry the assertion via [`ReturnRewriter`]. Returns the tokens to
/// emit instead when the body has a `return` the rewrite cannot reach.
fn instrument_exit_paths(
    input_fn: &mut ItemFn,
    prelude: proc_macro2::TokenStream,
    assertion: proc_macro2::TokenStream,
) -> Option<TokenStream> {
    let mut rewriter = ReturnRewriter {
        assertion: assertion.clone(),
        macro_returns: Vec::new(),
    };
    let original_fn = input_fn.clone();
    rewriter.visit_block_mut(&mut input_fn.block);

    if !rewriter.macro_returns.is_empty() {
        let errors = rewriter
            .macro_returns
            .iter()
            .map(|span| syn::Error::new(*span, MACRO_RETURN_ERROR).to_compile_error());
        // Emit the untouched function too, so the only error reported is ours.
        return Some(TokenStream::from(quote! {
            #(#errors)*
            #original_fn
        }));
    }

    // A trailing expression is the function's value (e.g. `Ok(())`), so it has to
    // be bound before the assertion and yielded afterwards. A trailing `return`
    // already carries the assertion from the rewrite above.
    let tail = match input_fn.block.stmts.last() {
        Some(syn::Stmt::Expr(expr, None)) if !matches!(expr, Expr::Return(_)) => {
            match input_fn.block.stmts.pop() {
                Some(syn::Stmt::Expr(expr, None)) => Some(expr),
                _ => unreachable!("last statement was just matched as a trailing expression"),
            }
        }
        _ => None,
    };

    let stmts = &input_fn.block.stmts;
    let new_block = match tail {
        Some(tail) => quote! {
            {
                #prelude

                #(#stmts)*

                let __budget_value = #tail;
                #assertion
                __budget_value
            }
        },
        None => quote! {
            {
                #prelude

                #(#stmts)*

                #assertion
            }
        },
    };

    *input_fn.block = match syn::parse2(new_block) {
        Ok(block) => block,
        Err(e) => return Some(TokenStream::from(e.into_compile_error())),
    };
    // A body ending in `return`/`panic!` makes the trailing assertion unreachable;
    // the paths that do reach an exit already asserted.
    input_fn
        .attrs
        .push(syn::parse_quote!(#[allow(unreachable_code)]));

    None
}

fn generate_prelude() -> proc_macro2::TokenStream {
    quote! {
        #[allow(unused_variables)]
        let budget_env_resolve = |var: &str| -> Option<String> {
            std::env::var(var).ok()
        };

        #[allow(unused_variables)]
        let parse_config_value = |content: &str, key: &str| -> Option<u64> {
            let key_pattern = format!("\"{}\"", key);
            let key_start = content.find(&key_pattern)?;
            let after_key = &content[key_start + key_pattern.len()..];
            let colon_pos = after_key.find(':')?;
            let after_colon = after_key[colon_pos + 1..].trim();
            let num_end = after_colon
                .find(|c: char| !c.is_ascii_digit() && c != ',' && c != '}')
                .unwrap_or(after_colon.len());
            let num_str = after_colon[..num_end]
                .trim()
                .trim_end_matches(',')
                .trim_end_matches('}')
                .trim_matches('"');
            num_str.parse().ok()
        };

        #[allow(unused_variables)]
        let parse_env_file_value = |content: &str, key: &str| -> Option<String> {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }
                let (lhs, rhs) = trimmed.split_once('=')?;
                if lhs.trim() == key {
                    let raw = rhs.trim();
                    // Strip inline comments ("  # provenance") that
                    // cargo budget-report --derive-limits appends to
                    // each KEY=VALUE line.
                    let raw = match raw.find(" #") {
                        Some(pos) => raw[..pos].trim(),
                        None => raw,
                    };
                    let unquoted = raw
                        .strip_prefix('"')
                        .and_then(|s| s.strip_suffix('"'))
                        .or_else(|| {
                            raw.strip_prefix('\'').and_then(|s| s.strip_suffix('\''))
                        })
                        .unwrap_or(raw);
                    return Some(unquoted.to_string());
                }
            }
            None
        };
    }
}

fn generate_budget_assert(spec: BudgetSpec, mut input_fn: ItemFn, fn_label: &str) -> TokenStream {
    let env_ident = spec
        .env_ident
        .unwrap_or_else(|| proc_macro2::Ident::new("env", proc_macro2::Span::call_site()));

    let mut asserts = Vec::new();

    if let Some(limit) = spec.cpu {
        let limit_expr = generate_limit_expr(&limit, "budget_cpu_lt");
        let cost_ident = proc_macro2::Ident::new("cpu_cost", proc_macro2::Span::call_site());
        let (msg, marginal_msg) = assert_messages(
            "CPU instruction cost",
            "local estimate, real network cost may differ significantly in either direction",
            &limit,
            fn_label,
        );
        asserts.push(generate_metric_assert(
            &cost_ident,
            quote! { budget.cpu_instruction_cost() },
            &limit_expr,
            spec.cpu_baseline.as_ref(),
            &msg,
            &marginal_msg,
        ));
    }

    if let Some(limit) = spec.mem {
        let limit_expr = generate_limit_expr(&limit, "budget_mem_lt");
        let cost_ident = proc_macro2::Ident::new("mem_cost", proc_macro2::Span::call_site());
        let (msg, marginal_msg) = assert_messages(
            "Memory bytes cost",
            "local estimate, real network cost may differ significantly in either direction",
            &limit,
            fn_label,
        );
        asserts.push(generate_metric_assert(
            &cost_ident,
            quote! { budget.memory_bytes_cost() },
            &limit_expr,
            spec.mem_baseline.as_ref(),
            &msg,
            &marginal_msg,
        ));
    }

    let prelude = generate_prelude();
    let assertion = quote! {
        {
            let budget = #env_ident.cost_estimate().budget();
            #(#asserts)*
        }
    };

    if let Some(tokens) = instrument_exit_paths(&mut input_fn, prelude, assertion) {
        return tokens;
    }

    TokenStream::from(quote! {
        #input_fn
    })
}

/// Asserts that the CPU instructions used by `env` are strictly less than a specified limit.
///
/// Must be placed on a test function that contains a local `env` variable (a `soroban_sdk::Env`).
/// The macro appends an assertion check to the body of the test function that measures
/// `env.cost_estimate().budget().cpu_instruction_cost()`.
///
/// # Local Estimates vs Network Costs
///
/// This attribute checks a **local estimate** of CPU instruction consumption.
/// Local estimates (such as raw Rust test execution or unoptimized local WASM builds) can
/// strictly underestimate or differ significantly from real Testnet or Futurenet costs, which
/// include host function overheads, VM metering, and protocol execution parameters.
///
/// Use local assertions as a fast local regression gate. For true network ground truth, use
/// `cargo budget-report`.
///
/// # Usage Examples
///
/// ## Static Limit
///
/// Pass an integer literal representing the maximum allowed CPU instructions:
///
/// ```rust,ignore
/// use budget_macros::budget_cpu_lt;
/// use soroban_sdk::Env;
///
/// #[test]
/// #[budget_cpu_lt(950_000)]
/// fn test_cpu_budget() {
///     let env = Env::default();
///     // ... setup contract client and invoke contract function ...
/// }
/// ```
///
/// ## Dynamic Limit via Environment Variable (`env = "VAR_NAME"`)
///
/// Read the limit dynamically from an environment variable at test runtime:
///
/// ```rust,ignore
/// use budget_macros::budget_cpu_lt;
/// use soroban_sdk::Env;
///
/// #[test]
/// #[budget_cpu_lt(env = "MAX_CPU_INSTRUCTIONS")]
/// fn test_cpu_budget_dynamic() {
///     let env = Env::default();
///     // ... setup contract client and invoke contract function ...
/// }
/// ```
///
/// When using `env = "VAR_NAME"`:
/// - If the environment variable is **unset**, the limit defaults to `u64::MAX` ("no limit"),
///   allowing the test assertion to pass unconditionally.
/// - If the environment variable is set to a string that **cannot be parsed as a `u64`**,
///   the test panics at runtime with an explicit error naming the variable and invalid value.
///
/// ## Limit from a `.env` File (`env_file = "PATH"` + `env = "VAR_NAME"`)
///
/// Read the limit from a `KEY=VALUE` file on disk at test runtime. This is the
/// **recommended form for Tier A limits derived from a Tier B report**: a single
/// checked-in `tier-a-limits.env` holds every limit the local test suite needs,
/// and each test reads exactly the keys it consumes. No `unsafe
/// std::env::set_var` is required — the file is parsed per-assertion, so the
/// mechanism is thread-safe and review-friendly (`git diff` shows exactly
/// which limit moved).
///
/// ```rust,ignore
/// use budget_macros::budget_cpu_lt;
/// use soroban_sdk::Env;
///
/// #[test]
/// #[budget_cpu_lt(env_file = "../tier-a-limits.env", env = "TIER_A__amm_pool__deposit__cpu")]
/// fn test_deposit_cpu_budget() {
///     let env = Env::default();
///     // ... setup and invoke ...
/// }
/// ```
///
/// When `env_file` is used:
/// - If the file cannot be read, the test panics with the file path.
/// - If the key is missing, the test panics with both the file path and the key.
/// - If the value cannot be parsed as `u64`, the test panics with the raw value.
///
/// Read the companion workflow in the project `README.md` (section
/// "Deriving Tier A limits from a Tier B report") for how to populate
/// `tier-a-limits.env`.
#[proc_macro_attribute]
pub fn budget_cpu_lt(attr: TokenStream, item: TokenStream) -> TokenStream {
    let spec = match syn::parse2::<StandaloneSpec>(attr.into()) {
        Ok(s) => s,
        Err(e) => return TokenStream::from(e.to_compile_error()),
    };
    expand_targets(item, |input_fn, fn_label| {
        generate_budget_assert(
            BudgetSpec {
                cpu: Some(spec.limit.clone()),
                mem: None,
                cpu_baseline: spec.baseline.clone(),
                mem_baseline: None,
                env_ident: None,
            },
            input_fn,
            fn_label,
        )
    })
}

/// Shared expansion for the two ledger-bytes proxy macros
/// (`budget_write_bytes_lt` / `budget_read_bytes_lt`), which differ only in
/// their metric label and failure phrasing. Both proxy the figure through
/// `memory_bytes_cost()`.
fn generate_bytes_proxy_assert(
    spec: StandaloneSpec,
    mut input_fn: ItemFn,
    metric_label: &str,
    metric_phrase: &str,
    fn_label: &str,
) -> TokenStream {
    let limit_expr = generate_limit_expr(&spec.limit, metric_label);

    let env_ident = proc_macro2::Ident::new("env", proc_macro2::Span::call_site());
    let cost_ident = proc_macro2::Ident::new("bytes_proxy_cost", proc_macro2::Span::call_site());
    let (plain, marginal) = assert_messages(
        metric_phrase,
        "local estimate, underestimates real network cost",
        &spec.limit,
        fn_label,
    );
    let assert_tokens = generate_metric_assert(
        &cost_ident,
        quote! { budget.memory_bytes_cost() },
        &limit_expr,
        spec.baseline.as_ref(),
        &plain,
        &marginal,
    );

    let prelude = generate_prelude();
    let assertion = quote! {
        {
            let budget = #env_ident.cost_estimate().budget();
            #assert_tokens
        }
    };

    if let Some(tokens) = instrument_exit_paths(&mut input_fn, prelude, assertion) {
        return tokens;
    }

    TokenStream::from(quote! {
        #input_fn
    })
}

/// Asserts that the ledger write bytes used by `env` are less than N.
///
/// Write bytes represent the total bytes written to ledger storage during
/// contract execution. This macro measures the local `memory_bytes_cost` as a
/// proxy, which correlates with storage serialization overhead even though the
/// exact on-network write-bytes figure is only available via RPC simulation.
/// Must be placed on a test function (or a budget-annotated `impl` block) that
/// has a local `env` variable.
#[proc_macro_attribute]
pub fn budget_write_bytes_lt(attr: TokenStream, item: TokenStream) -> TokenStream {
    let spec = match syn::parse::<StandaloneSpec>(attr) {
        Ok(s) => s,
        Err(e) => return TokenStream::from(e.to_compile_error()),
    };
    expand_targets(item, |input_fn, fn_label| {
        generate_bytes_proxy_assert(
            spec.clone(),
            input_fn,
            "budget_write_bytes_lt",
            "Write bytes cost (memory proxy)",
            fn_label,
        )
    })
}

/// Asserts that the ledger read bytes used by `env` are less than N.
///
/// Read bytes represent the total bytes read from ledger storage during
/// contract execution. This macro measures the local `memory_bytes_cost` as a
/// proxy, which correlates with storage access overhead even though the
/// exact on-network read-bytes figure is only available via RPC simulation.
/// Must be placed on a test function that has a local `env` variable.
///
/// # Local Estimates vs Network Costs
///
/// This attribute checks a **local estimate** of read byte consumption.
/// Local estimates can underestimate real Testnet or Futurenet costs.
/// Use local assertions as a fast local regression gate. For true network ground truth,
/// use `cargo budget-report`.
///
/// # Usage Examples
///
/// ## Static Limit
///
/// ```rust,ignore
/// use budget_macros::budget_read_bytes_lt;
/// use soroban_sdk::Env;
///
/// #[test]
/// #[budget_read_bytes_lt(4_096)]
/// fn test_read_bytes_budget() {
///     let env = Env::default();
///     // ...
/// }
/// ```
///
/// ## Dynamic Limit via Environment Variable (`env = "VAR_NAME"`)
///
/// ```rust,ignore
/// use budget_macros::budget_read_bytes_lt;
/// use soroban_sdk::Env;
///
/// #[test]
/// #[budget_read_bytes_lt(env = "MAX_READ_BYTES")]
/// fn test_read_bytes_dynamic() {
///     let env = Env::default();
/// }
/// ```
///
/// ## Limit from a `.env` File (`env_file = "PATH"` + `env = "VAR_NAME"`)
///
/// ```rust,ignore
/// use budget_macros::budget_read_bytes_lt;
/// use soroban_sdk::Env;
///
/// #[test]
/// #[budget_read_bytes_lt(env_file = "../tier-a-limits.env", env = "TIER_A__AMM_POOL_CONTRACT__DEPOSIT__READ")]
/// fn test_deposit_read_budget() {
///     let env = Env::default();
/// }
/// ```
#[proc_macro_attribute]
pub fn budget_read_bytes_lt(attr: TokenStream, item: TokenStream) -> TokenStream {
    let spec = match syn::parse::<StandaloneSpec>(attr) {
        Ok(s) => s,
        Err(e) => return TokenStream::from(e.to_compile_error()),
    };
    expand_targets(item, |input_fn, fn_label| {
        generate_bytes_proxy_assert(
            spec.clone(),
            input_fn,
            "budget_read_bytes_lt",
            "Read bytes cost (memory proxy)",
            fn_label,
        )
    })
}

/// Asserts that the number of events emitted by `env` stays under `N`.
///
/// Events are metered by Soroban and are the resource most likely to grow
/// accidentally: a contract that emits one event per loop iteration passes
/// every CPU and memory assertion while producing an unbounded number of
/// events, and nothing else in the macro set catches it.
///
/// The count is obtained **directly** from the SDK's test environment via
/// `env.events().all().events().len()` — it is a real event count, not a proxy
/// for another metric. (Confirmed: `soroban_sdk::Env::events()` returns an
/// `Events` whose `all()` yields the emitted `ContractEvent`s, so the count is
/// exact under `feature = "testutils"`.)
///
/// Supports the same limit forms as the other macros — literal, `env = "VAR"`,
/// `env_file = "PATH", env = "KEY"`, `config = "KEY"`, and `pct = N, of = …`.
///
/// Must be placed on a test function that has a local `env` variable (a
/// `soroban_sdk::Env`).
#[proc_macro_attribute]
pub fn budget_events_lt(attr: TokenStream, item: TokenStream) -> TokenStream {
    let spec = match syn::parse::<StandaloneSpec>(attr) {
        Ok(s) => s,
        Err(e) => return TokenStream::from(e.to_compile_error()),
    };
    let mut input_fn = match syn::parse::<ItemFn>(item) {
        Ok(f) => f,
        Err(e) => return TokenStream::from(e.to_compile_error()),
    };

    let limit_expr = generate_limit_expr(&spec.limit, "budget_events_lt");

    let env_ident = proc_macro2::Ident::new("env", proc_macro2::Span::call_site());
    let cost_ident = proc_macro2::Ident::new("event_count", proc_macro2::Span::call_site());
    let assert_tokens = generate_metric_assert(
        &cost_ident,
        quote! { #env_ident.events().all().events().len() as u64 },
        &limit_expr,
        spec.baseline.as_ref(),
        "Event count {} exceeded limit {} - local estimate, real network event counts may differ",
        "Event count {} exceeded limit {} (marginal: {} measured - {} baseline) - local estimate, real network event counts may differ",
    );

    let prelude = generate_prelude();
    let assertion = quote! {
        {
            #assert_tokens
        }
    };

    if let Some(tokens) = instrument_exit_paths(&mut input_fn, prelude, assertion) {
        return tokens;
    }

    TokenStream::from(quote! {
        #input_fn
    })
}

/// Asserts that the number of ledger entries accessed by `env` stays under `N`.
///
/// Soroban limits the number of ledger entries a transaction may read or write
/// separately from the byte counts. A contract can sit well inside its read and
/// write byte budgets while touching too many distinct entries — reading fifty
/// small entries is cheap in bytes and expensive in entry count.
///
/// The total asserted is **reads + writes** (the network enforces a single
/// combined entry limit), but the failure message always reports the read and
/// write breakdown so a breach is never ambiguous about which side blew the
/// budget. Read and write are deliberately summed rather than silently picked:
/// the macro reports both.
///
/// Counts come from `env.cost_estimate().budget().tracker(ContractCostType::DiskReadEntries)`
/// and `…DiskWriteEntries`, summed. `ContractCostType` must be in scope at the
/// call site (e.g. `use soroban_sdk::ContractCostType;`).
///
/// Supports the same limit forms as the other macros.
///
/// Must be placed on a test function that has a local `env` variable (a
/// `soroban_sdk::Env`).
#[proc_macro_attribute]
pub fn budget_ledger_entries_lt(attr: TokenStream, item: TokenStream) -> TokenStream {
    let spec = match syn::parse::<StandaloneSpec>(attr) {
        Ok(s) => s,
        Err(e) => return TokenStream::from(e.to_compile_error()),
    };
    let mut input_fn = match syn::parse::<ItemFn>(item) {
        Ok(f) => f,
        Err(e) => return TokenStream::from(e.to_compile_error()),
    };

    let limit_expr = generate_limit_expr(&spec.limit, "budget_ledger_entries_lt");

    let env_ident = proc_macro2::Ident::new("env", proc_macro2::Span::call_site());
    let cost_ident = proc_macro2::Ident::new("ledger_entries", proc_macro2::Span::call_site());
    let assert_tokens =
        generate_ledger_assert(&cost_ident, &env_ident, &limit_expr, spec.baseline.as_ref());

    let prelude = generate_prelude();
    let assertion = quote! {
        {
            #assert_tokens
        }
    };

    if let Some(tokens) = instrument_exit_paths(&mut input_fn, prelude, assertion) {
        return tokens;
    }

    TokenStream::from(quote! {
        #input_fn
    })
}

/// Asserts that the memory bytes used by `env` are strictly less than a specified limit.
///
/// Must be placed on a test function that contains a local `env` variable (a `soroban_sdk::Env`).
/// The macro appends an assertion check to the body of the test function that measures
/// `env.cost_estimate().budget().memory_bytes_cost()`.
///
/// # Local Estimates vs Network Costs
///
/// This attribute checks a **local estimate** of memory byte consumption.
/// Local estimates (such as raw Rust test execution or unoptimized local WASM builds) can
/// strictly underestimate or differ significantly from real Testnet or Futurenet costs, which
/// include host function overheads, VM heap/stack allocation overheads, and protocol execution parameters.
///
/// Use local assertions as a fast local regression gate. For true network ground truth, use
/// `cargo budget-report`.
///
/// # Usage Examples
///
/// ## Static Limit
///
/// Pass an integer literal representing the maximum allowed memory bytes:
///
/// ```rust,ignore
/// use budget_macros::budget_mem_lt;
/// use soroban_sdk::Env;
///
/// #[test]
/// #[budget_mem_lt(500_000)]
/// fn test_memory_budget() {
///     let env = Env::default();
///     // ... setup contract client and invoke contract function ...
/// }
/// ```
///
/// ## Dynamic Limit via Environment Variable (`env = "VAR_NAME"`)
///
/// Read the limit dynamically from an environment variable at test runtime:
///
/// ```rust,ignore
/// use budget_macros::budget_mem_lt;
/// use soroban_sdk::Env;
///
/// #[test]
/// #[budget_mem_lt(env = "MAX_MEMORY_BYTES")]
/// fn test_memory_budget_dynamic() {
/// fn test_mem_budget_dynamic() {
///     let env = Env::default();
///     // ... setup contract client and invoke contract function ...
/// }
/// ```
///
/// When using `env = "VAR_NAME"`:
/// ## Limit from a `.env` File (`env_file = "PATH"` + `env = "VAR_NAME"`)
///
/// Same as the `budget_cpu_lt` form: the limit is read at test runtime from
/// the `KEY=VALUE` file at `PATH`. See `budget_cpu_lt`'s documentation and
/// the project `README.md` for the derivation workflow.
///
/// When using `env = "VAR_NAME"` (no `env_file`):
/// - If the environment variable is **unset**, the limit defaults to `u64::MAX` ("no limit"),
///   allowing the test assertion to pass unconditionally.
/// - If the environment variable is set to a string that **cannot be parsed as a `u64`**,
///   the test panics at runtime with an explicit error naming the variable and invalid value.
#[proc_macro_attribute]
pub fn budget_mem_lt(attr: TokenStream, item: TokenStream) -> TokenStream {
    let spec = match syn::parse2::<StandaloneSpec>(attr.into()) {
        Ok(s) => s,
        Err(e) => return TokenStream::from(e.to_compile_error()),
    };
    expand_targets(item, |input_fn, fn_label| {
        generate_budget_assert(
            BudgetSpec {
                cpu: None,
                mem: Some(spec.limit.clone()),
                cpu_baseline: None,
                mem_baseline: spec.baseline.clone(),
                env_ident: None,
            },
            input_fn,
            fn_label,
        )
    })
}

/// Asserts that the CPU and/or memory bytes used by `env` are less than specified limits.
/// Must be placed on a test function that has a local `env` variable.
///
/// Limits can be specified as `cpu = N` and `mem = M`. The same four
/// `(integer | env | env_file + env | config)` forms accepted by
/// `budget_cpu_lt` work here for each metric.
///
/// This checks a *local* estimate. Real network cost can differ from it
/// significantly in either direction depending on the build profile — see
/// `docs/src/mechanics.md` for measurements. Use `cargo budget-report` for
/// network ground truth, and `cargo budget-report --derive-limits` to
/// regenerate Tier A limits from a fresh Tier B report.
#[proc_macro_attribute]
pub fn budget_lt(attr: TokenStream, item: TokenStream) -> TokenStream {
    let spec = match syn::parse2::<BudgetSpec>(attr.into()) {
        Ok(s) => s,
        Err(e) => return TokenStream::from(e.to_compile_error()),
    };
    expand_targets(item, |input_fn, fn_label| {
        generate_budget_assert(spec.clone(), input_fn, fn_label)
    })
}

// ---------------------------------------------------------------------------
// Scaling assertion: `#[budget_scaling(…)]`
// ---------------------------------------------------------------------------

/// Supported growth models for the scaling assertion.
#[derive(Clone)]
enum GrowthModel {
    Linear,
    Quadratic,
}

impl GrowthModel {
    fn as_str(&self) -> &'static str {
        match self {
            GrowthModel::Linear => "linear",
            GrowthModel::Quadratic => "quadratic",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            GrowthModel::Linear => "linear (cost ∝ n)",
            GrowthModel::Quadratic => "quadratic (cost ∝ n²)",
        }
    }
}

impl Parse for GrowthModel {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident: Ident = input.parse()?;
        match ident.to_string().as_str() {
            "linear" => Ok(GrowthModel::Linear),
            "quadratic" => Ok(GrowthModel::Quadratic),
            other => Err(syn::Error::new(
                ident.span(),
                format!("unknown growth model `{other}`, expected `linear` or `quadratic`"),
            )),
        }
    }
}

/// Configuration for the `#[budget_scaling(…)]` attribute.
struct ScalingConfig {
    sizes: Vec<u32>,
    model: GrowthModel,
    tolerance: f64,
}

impl Parse for ScalingConfig {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut sizes: Option<Vec<u32>> = None;
        let mut model: Option<GrowthModel> = None;
        let mut tolerance: Option<f64> = None;

        while !input.is_empty() {
            let ident: Ident = input.parse()?;
            input.parse::<Token![=]>()?;

            match ident.to_string().as_str() {
                "sizes" => {
                    let content;
                    syn::bracketed!(content in input);
                    let mut values = Vec::new();
                    while !content.is_empty() {
                        let lit: LitInt = content.parse()?;
                        values.push(lit.base10_parse()?);
                        if !content.is_empty() {
                            let _ = content.parse::<Token![,]>();
                        }
                    }
                    sizes = Some(values);
                }
                "model" => {
                    model = Some(input.parse()?);
                }
                "tolerance" => {
                    let lit: LitFloat = input.parse()?;
                    tolerance = Some(lit.base10_parse()?);
                }
                other => {
                    return Err(syn::Error::new(
                        ident.span(),
                        format!(
                            "unknown scaling property `{other}`, \
                             expected `sizes`, `model`, or `tolerance`"
                        ),
                    ));
                }
            }

            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }

        let sizes = sizes.ok_or_else(|| input.error("missing required field `sizes`"))?;
        if sizes.len() < 2 {
            return Err(syn::Error::new(
                Span::call_site(),
                "`sizes` must contain at least two elements for ratio comparison",
            ));
        }
        Ok(ScalingConfig {
            sizes,
            model: model.ok_or_else(|| input.error("missing required field `model`"))?,
            tolerance: tolerance
                .ok_or_else(|| input.error("missing required field `tolerance`"))?,
        })
    }
}

fn generate_scaling_assert(config: ScalingConfig, input_fn: ItemFn) -> TokenStream {
    let name = &input_fn.sig.ident;
    let body = &input_fn.block;
    let vis = &input_fn.vis;

    // Preserve user attributes (e.g. #[should_panic]) and add #[test] if needed.
    let mut attrs: Vec<Attribute> = input_fn.attrs.clone();
    let has_test = attrs.iter().any(|a| a.path().is_ident("test"));
    if !has_test {
        attrs.push(parse_quote!(#[test]));
    }

    let sizes_values = &config.sizes;
    let tolerance = config.tolerance;
    let model_name = config.model.as_str();
    let model_desc = config.model.description();

    let ratio_expr = match config.model {
        GrowthModel::Linear => {
            quote! { __curr_s as f64 / __prev_s as f64 }
        }
        GrowthModel::Quadratic => {
            quote! { (__curr_s as f64 / __prev_s as f64).powi(2) }
        }
    };

    // Build the format string at compile time so model_name and model_desc are
    // baked directly into the string rather than looked up at runtime.
    let assert_msg = format!(
        "Scaling check failed at size {{__curr_s}}:\n\
         \tExpected ratio: ~{{__expected_ratio:.2}} (model = {model_name})\n\
         \tObserved ratio: ~{{__observed_ratio:.2}}\n\
         \tDeviation: {{__deviation:.2}} > tolerance {{__TOLERANCE:.2}}\n\
         \tMeasured sizes:  {{:?}}\n\
         \tMeasured costs:  {{:?}}\n\
         \tExpected growth: {model_desc}",
    );
    let assert_msg_lit = syn::LitStr::new(&assert_msg, Span::call_site());

    TokenStream::from(quote! {
        #(#attrs)*
        #vis fn #name() {
            const __SIZES: &[u32] = &[#(#sizes_values),*];
            const __TOLERANCE: f64 = #tolerance;

            let mut __measurements: ::std::vec::Vec<(u32, u64)> =
                ::std::vec::Vec::new();

            for &size in __SIZES {
                let env = soroban_sdk::Env::default();
                env.cost_estimate().budget().reset_unlimited();

                #body

                let __cost = env.cost_estimate().budget()
                    .cpu_instruction_cost();
                __measurements.push((size, __cost));
            }

            for i in 1..__measurements.len() {
                let (__prev_s, __prev_c) = __measurements[i - 1];
                let (__curr_s, __curr_c) = __measurements[i];

                let __expected_ratio = #ratio_expr;
                let __observed_ratio = __curr_c as f64 / __prev_c as f64;
                let __deviation =
                    (__observed_ratio / __expected_ratio - 1.0).abs();

                assert!(
                    __deviation <= __TOLERANCE,
                    #assert_msg_lit,
                    __measurements
                        .iter()
                        .map(|(s, _)| s)
                        .copied()
                        .collect::<::std::vec::Vec<u32>>(),
                    __measurements
                        .iter()
                        .map(|(_, c)| c)
                        .copied()
                        .collect::<::std::vec::Vec<u64>>(),
                );
            }
        }
    })
}

/// Asserts that the budget cost grows according to a declared model across
/// multiple input sizes.
///
/// This is not a single-point assertion.  Instead it measures execution cost
/// for each caller-provided input size and validates that the observed cost
/// growth stays within a configurable tolerance of the declared model (e.g.
/// `linear` or `quadratic`).
///
/// # Attribute syntax
///
/// ```rust,ignore
/// #[budget_scaling(
///     sizes = [10, 100, 1000],
///     model = linear,
///     tolerance = 0.3,
/// )]
/// fn my_operation(env: Env, size: u32) {
///     // body using `env` and `size` — runs once per input size
/// }
/// ```
///
/// | Field       | Type              | Description |
/// |-------------|-------------------|-------------|
/// | `sizes`     | `[u32; N]`        | Input sizes to measure (at least 2). |
/// | `model`     | `linear` / `quadratic` | Expected growth model. |
/// | `tolerance` | `f64`             | Allowed relative deviation from expected ratio (e.g. `0.3` = 30 %). |
///
/// The annotated function becomes a `#[test]` function.  The macro:
///
/// 1. Creates a fresh `Env` for each size.
/// 2. Resets the budget and runs the function body.
/// 3. Records `cpu_instruction_cost()`.
/// 4. Compares consecutive (size, cost) pairs against the model.
///
/// # Growth model
///
/// The check uses successive ratio comparisons:
///
/// - **Linear** — cost is expected to grow proportionally to the input
///   (`cost ∝ n`). The expected cost ratio between two sizes is
///   `size_{i+1} / size_i`.
/// - **Quadratic** — cost is expected to grow as the square of the input
///   (`cost ∝ n²`). The expected cost ratio is
///   `(size_{i+1} / size_i)²`.
///
/// The observed ratio `cost_{i+1} / cost_i` is compared to the expected
/// ratio.  If the absolute relative deviation exceeds `tolerance`, the
/// assertion panics with a diagnostic that lists:
///
/// - the offending size
/// - expected ratio
/// - observed ratio
/// - deviation vs tolerance
/// - all measured sizes and costs
/// - expected growth description
///
/// # Tolerance
///
/// `tolerance` is the maximum allowed `|observed / expected - 1|`.  A
/// tolerance of `0.3` means the observed ratio may deviate up to 30 % from
/// the expected ratio.
///
/// # Limitations
///
/// - The function body **must not** contain `return`, `break`, or `continue`
///   that would exit the measurement loop prematurely.
/// - Each iteration creates a fresh `Env` — setup that must survive across
///   sizes should be placed outside the macro (e.g. by extracting it to a
///   helper called from the body).
/// - Because the check relies on ratio comparisons, small base costs (Env
///   creation, budget reset) can dominate and mask the growth signal at very
///   small sizes.  Use sizes large enough that the measured work dominates.
/// - Only CPU instruction cost is checked.  Memory scaling is not yet
///   supported.
#[proc_macro_attribute]
pub fn budget_scaling(attr: TokenStream, item: TokenStream) -> TokenStream {
    let config = match syn::parse2::<ScalingConfig>(attr.into()) {
        Ok(c) => c,
        Err(e) => return TokenStream::from(e.to_compile_error()),
    };

    let input_fn = match syn::parse2::<ItemFn>(item.into()) {
        Ok(f) => f,
        Err(e) => return TokenStream::from(e.to_compile_error()),
    };

    generate_scaling_assert(config, input_fn)
}

#[cfg(test)]
mod tests {
    use super::resolve_env_file_at_expansion;

    #[test]
    fn resolves_a_path_relative_to_the_crate_manifest_dir() {
        // Cargo sets CARGO_MANIFEST_DIR to `budget-macros/` for this crate's
        // own tests, and `Cargo.toml` is guaranteed to sit there.
        assert!(resolve_env_file_at_expansion("Cargo.toml").is_some());
    }

    #[test]
    fn resolves_the_checked_in_ui_fixture_env_file() {
        assert!(
            resolve_env_file_at_expansion("tests/ui/support/pass_env_file.env").is_some(),
            "the UI env_file fixture should resolve from the crate manifest dir"
        );
    }

    #[test]
    fn a_missing_path_resolves_to_none() {
        assert!(resolve_env_file_at_expansion("definitely/not/a/real/limits.env").is_none());
    }

    #[test]
    fn a_directory_is_not_accepted_as_an_env_file() {
        // `is_file()` must gate every candidate, so `src` (a directory) is a miss.
        assert!(resolve_env_file_at_expansion("src").is_none());
    }
}
