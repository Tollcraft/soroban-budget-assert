extern crate proc_macro;

use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{parse::Parse, parse::ParseStream, Expr, Ident, ItemFn, LitInt, LitStr, Token};

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
}

#[derive(Default)]
struct BudgetSpec {
    cpu: Option<BudgetLimit>,
    mem: Option<BudgetLimit>,
    env_ident: Option<Ident>,
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

        // The leading form may also be a bare integer literal. Detect that
        // case before parsing identifiers.
        if input.peek(LitInt) {
            let lit: LitInt = input.parse()?;
            // If anything follows, it must be key=value pairs (e.g.
            // `950_000, env_file = "..."`) — but we currently don't model
            // mixed literal+modifier forms. Reject to keep the rule
            // simple: a literal stands alone.
            if !input.is_empty() {
                return Err(syn::Error::new(
                    lit.span(),
                    "integer literal cannot be combined with env / config / env_file",
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
                        "env" | "env_file" | "config"
                    ))
                {
                    break;
                }
                input.parse::<Token![,]>()?;
            } else if input.peek(Ident) {
                let ahead = input.fork();
                let key: Ident = ahead.parse().unwrap();
                if !matches!(key.to_string().as_str(), "env" | "env_file" | "config") {
                    break;
                }
            } else {
                break;
            }

            let ident: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let ident_str = ident.to_string();
            if ident_str == "env_file" {
                // Accept either a string literal or an identifier/const path
                // for env_file, so callers can write `env_file = "path"` or
                // `env_file = CONST_NAME`.
                let path: proc_macro2::TokenStream = if input.peek(LitStr) {
                    let lit: LitStr = input.parse()?;
                    lit.into_token_stream()
                } else {
                    let expr: Expr = input.parse()?;
                    expr.into_token_stream()
                };
                env_file = Some(path);
            } else {
                let lit: LitStr = input.parse()?;
                match ident_str.as_str() {
                    "env" => env_var = Some(lit.value()),
                    "config" => config_key = Some(lit.value()),
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
        // Precedence: `env_file` + `env` → `EnvFile`; `env` only → `EnvVar`;
        // `config` → `Config`. Mixing types is rejected to avoid silent
        // confusion: `env_file` is meaningless without a key, and `env` is
        // meaningless without a file when `env_file` is set.
        match (env_file, env_var, config_key) {
            (Some(path), Some(var), None) => Ok(BudgetLimit::EnvFile {
                path,
                var_name: var,
            }),
            (None, Some(var), None) => Ok(BudgetLimit::EnvVar(var)),
            (None, None, Some(key)) => Ok(BudgetLimit::Config(key)),
            (Some(_), None, _) | (Some(_), _, Some(_)) => Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "`env_file` must be paired with `env = \"VAR_NAME\"` and not with `config`",
            )),
            (None, Some(_), Some(_)) => Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "`env` and `config` cannot be combined; pick one",
            )),
            (None, None, None) => Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "expected an integer literal, `env = \"VAR\"`, `env_file = \"PATH\"` + `env = \"VAR\"`, or `config = \"KEY\"`",
            )),
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
        // ── JSON-config limit resolution ──────────────────────────────
        // Reads `budget.json` from the current working directory and
        // extracts the value for the given key.  If the file is absent,
        // empty, malformed, or the key is not found, the limit silently
        // falls back to `u64::MAX` ("no limit") — the test assertion
        // passes but does not enforce a ceiling.  This prevents a broken
        // or missing JSON file from crashing an otherwise-valid test
        // suite.
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
                    let resolved = std::fs::read_to_string(env_file_path).ok().and_then(|content| {
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
                        panic!(
                            "{}: env_file {} missing key {} (or file cannot be read)",
                            #metric_label,
                            env_file_path,
                            env_file_key,
                        )
                    })
                }
            }
        }
    }
}

fn generate_budget_assert(spec: BudgetSpec, item: TokenStream) -> TokenStream {
    let mut input_fn = match syn::parse2::<ItemFn>(item.into()) {
        Ok(f) => f,
        Err(e) => return TokenStream::from(e.to_compile_error()),
    };

    let stmts = &input_fn.block.stmts;
    let env_ident = spec
        .env_ident
        .unwrap_or_else(|| proc_macro2::Ident::new("env", proc_macro2::Span::call_site()));

    let mut asserts = Vec::new();

    if let Some(limit) = spec.cpu {
        let limit_expr = generate_limit_expr(&limit, "budget_cpu_lt");
        let cost_ident = proc_macro2::Ident::new("cpu_cost", proc_macro2::Span::call_site());
        let cost_expr = quote! { budget.cpu_instruction_cost() };
        let assert_msg = "CPU instruction cost {} exceeded limit {} - local estimate, real network cost may differ significantly in either direction";
        asserts.push(quote! {
            let #cost_ident = #cost_expr;
            let limit_u64: u64 = #limit_expr;
            assert!(
                #cost_ident < limit_u64,
                #assert_msg,
                #cost_ident,
                limit_u64
            );
        });
    }

    if let Some(limit) = spec.mem {
        let limit_expr = generate_limit_expr(&limit, "budget_mem_lt");
        let cost_ident = proc_macro2::Ident::new("mem_cost", proc_macro2::Span::call_site());
        let cost_expr = quote! { budget.memory_bytes_cost() };
        let assert_msg = "Memory bytes cost {} exceeded limit {} - local estimate, real network cost may differ significantly in either direction";
        asserts.push(quote! {
            let #cost_ident = #cost_expr;
            let limit_u64: u64 = #limit_expr;
            assert!(
                #cost_ident < limit_u64,
                #assert_msg,
                #cost_ident,
                limit_u64
            );
        });
    }

    let prelude = quote! {
        #[allow(unused_variables)]
        let budget_env_resolve = |var: &str| -> Option<String> {
            std::env::var(var).ok()
        };
    };

            // Wrap injected temporaries in their own scope so they never
            // collide with user-declared `budget`, `cpu_cost`, `mem_cost`,
            // or `limit_u64` names in the test function body.
            {
                let budget = #env_ident.cost_estimate().budget();
                #(#asserts)*
            }
    // Injected at every path that leaves the test, not just after the last
    // statement, so an early `return` cannot skip it. It stays inside the body's
    // scope so each limit still resolves against the body's own bindings — tests
    // that shadow `budget_env_resolve` rely on that.
    let assertion = quote! {
    let new_block = quote! {
        {
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

            /// Parse a `KEY=VALUE` line out of an `.env`-shaped file body.
            ///
            /// Mirrors the shape used by `cargo budget-report
            /// --derive-limits` and other `.env` producers: one
            /// `KEY=VALUE` per non-comment, non-blank line, with `#`-led
            /// comment lines and surrounding whitespace tolerated.
            #[allow(unused_variables)]
            let parse_env_file_value = |content: &str, key: &str| -> Option<String> {
                for line in content.lines() {
                    let trimmed = line.trim();
                    if trimmed.is_empty() || trimmed.starts_with('#') {
                        continue;
                    }
                    let (lhs, rhs) = trimmed.split_once('=')?;
                    if lhs.trim() == key {
                        // Strip optional surrounding quotes from the value;
                        // `.env` producers commonly emit either form.
                        let raw = rhs.trim();
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

            #(#stmts)*

            let budget = #env_ident.cost_estimate().budget();
            #(#asserts)*
        }
    };

    *input_fn.block = match syn::parse2(new_block) {
        Ok(block) => block,
        Err(e) => return TokenStream::from(e.into_compile_error()),
    };

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
    let limit = match syn::parse2::<BudgetLimit>(attr.into()) {
        Ok(l) => l,
        Err(e) => return TokenStream::from(e.to_compile_error()),
    };
    generate_budget_assert(
        BudgetSpec {
            cpu: Some(limit),
            mem: None,
            env_ident: None,
        },
        item,
    )
}

/// Asserts that the ledger write bytes used by `env` are less than N.
///
/// Write bytes represent the total bytes written to ledger storage during
/// contract execution. This macro measures the local `memory_bytes_cost` as a
/// proxy, which correlates with storage serialization overhead even though the
/// exact on-network write-bytes figure is only available via RPC simulation.
/// Must be placed on a test function that has a local `env` variable.
#[proc_macro_attribute]
pub fn budget_write_bytes_lt(attr: TokenStream, item: TokenStream) -> TokenStream {
    let limit = match syn::parse::<BudgetLimit>(attr) {
        Ok(l) => l,
        Err(e) => return TokenStream::from(e.to_compile_error()),
    };
    let mut input_fn = match syn::parse::<ItemFn>(item) {
        Ok(f) => f,
        Err(e) => return TokenStream::from(e.to_compile_error()),
    };

    let stmts = &input_fn.block.stmts;

    let limit_expr = match limit {
        BudgetLimit::Int(n) => quote! { #n },
        BudgetLimit::EnvVar(var) => quote! {
            std::env::var(#var)
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(u64::MAX)
        },
        BudgetLimit::Config(key) => quote! {
            std::fs::read_to_string(std::path::Path::new("budget.json"))
                .ok()
                .map(|content| {
                    parse_config_value(&content, #key).unwrap_or_else(|| {
                        panic!(
                            "budget_write_bytes_lt: key '{}' not found or invalid in budget.json",
                            #key,
                        )
                    })
                })
                .unwrap_or(u64::MAX)
        },
    };

    let env_ident = proc_macro2::Ident::new("env", proc_macro2::Span::call_site());

    let new_block = quote! {
        {
            #(#stmts)*

            let budget = #env_ident.cost_estimate().budget();
            let write_bytes_cost = budget.memory_bytes_cost();
            let limit_u64: u64 = #limit_expr;
            assert!(
                write_bytes_cost < limit_u64,
                "Write bytes cost (memory proxy) {} exceeded limit {} - local estimate, underestimates real network cost",
                write_bytes_cost,
                limit_u64
            );
        }
    };

    *input_fn.block = syn::parse2(new_block).unwrap();

    TokenStream::from(quote! {
        #input_fn
    })
}
#[proc_macro_attribute]
pub fn budget_mem_lt(attr: TokenStream, item: TokenStream) -> TokenStream {
    let limit = match syn::parse2::<BudgetLimit>(attr.into()) {
        Ok(l) => l,
        Err(e) => return TokenStream::from(e.to_compile_error()),
    };
    generate_budget_assert(
        BudgetSpec {
            cpu: None,
            mem: Some(limit),
            env_ident: None,
        },
        item,
    )
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
    generate_budget_assert(spec, item)
}
