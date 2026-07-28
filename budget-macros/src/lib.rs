extern crate proc_macro;

use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{parse::Parse, parse::ParseStream, Expr, Ident, ItemFn, LitInt, LitStr, Token};

/// The resolved budget limit supplied by the user to a `budget_cpu_lt` or
/// `budget_mem_lt` attribute macro.
///
/// There are three forms, each of which is parsed from the attribute's token
/// stream by the [`Parse`] implementation:
///
/// | Form           | Syntax                         | Resolution                              |
/// |----------------|--------------------------------|-----------------------------------------|
/// | **Static**     | `#[budget_cpu_lt(500_000)]`    | Hard-coded `u64` literal at compile time|
/// | **Env variable**| `#[budget_cpu_lt(env = "VAR")]`| Read `std::env::var("VAR")` at runtime  |
/// | **JSON config**| `#[budget_cpu_lt(config = "key")]` | Parse `budget.json` at runtime      |
///
/// # Semantics
///
/// * `Int(n)` — the simplest form. The assertion checks that the measured cost
///   is strictly less than `n`.
/// * `EnvVar(name)` — looks up the environment variable `name` at test runtime.
///   If the variable is unset, the limit silently defaults to `u64::MAX`
///   (effectively "no limit") so CI can run without setting every variable.
///   If the variable is set but does not parse as a `u64`, the test panics with
///   an explicit message that includes the variable name and invalid value.
/// * `Config(key)` — reads a local `budget.json` file that must contain a
///   top-level `"key": <u64_value>` entry. If the file does not exist, the
///   limit defaults to `u64::MAX` ("no limit"). If the file exists but the key
///   is missing or the value is not a valid `u64`, the test panics.
enum BudgetLimit {
    /// A literal integer limit provided directly in the attribute.
    Int(u64),
    /// An environment variable name whose runtime value will be used as the limit.
    ///
    /// TODO: Add support for parsing a default value if the env var is missing,
    /// e.g. `env = "VAR" default = 500_000`.
    EnvVar(String),
    /// A JSON key in `budget.json` whose value will be used as the limit.
    Config(String),
}

/// The cost metric that a budget macro asserts against.
///
/// Determines which Soroban budget measurement is read and what assertion
/// message is produced on failure.
///
/// | Variant               | Macro             | Measurement                                |
/// |-----------------------|-------------------|--------------------------------------------|
/// | `CpuInstructionCost`  | `budget_cpu_lt`   | `env.cost_estimate().budget().cpu_instruction_cost()` |
/// | `MemoryBytesCost`     | `budget_mem_lt`   | `env.cost_estimate().budget().memory_bytes_cost()`    |
enum BudgetMetric {
    /// CPU instruction count consumed by the test function's `env`.
    CpuInstructionCost,
    /// Memory bytes consumed by the test function's `env`.
    MemoryBytesCost,
}

/// Parses a budget limit from the attribute macro's token stream.
///
/// Recognises three forms in order:
///
/// 1. **Ident = string** — `env = "VAR_NAME"` or `config = "key"`.
///    The identifier must be exactly `env` or `config`; any other identifier
///    produces a compile error with a descriptive span.
/// 2. **Integer literal** — a bare `u64` literal such as `500_000`.
///
/// # Errors
///
/// Returns a `syn::Error` with a helpful span if the input does not match
/// either form, including when an unrecognised identifier is used.
///
/// # Note
///
/// Support for a default value when an env var is missing is not yet
/// implemented (see the `TODO` on the [`EnvVar`](BudgetLimit::EnvVar) variant).
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

/// Generates the final token stream for a budget assertion attribute macro.
///
/// This is the shared code-generation path behind both [`budget_cpu_lt`] and
/// [`budget_mem_lt`]. It performs the following steps:
///
/// 1. **Parse the attribute** — attempts to parse the token stream as a
///    [`BudgetLimit`]. On failure, emits a compile error at the call site.
/// 2. **Parse the item** — attempts to parse the annotated item as a function
///    ([`syn::ItemFn`]). On failure, emits a compile error.
/// 3. **Generate limit-resolution code** — depending on the variant of
///    [`BudgetLimit`] (`Int`, `EnvVar`, or `Config`), emits the appropriate
///    runtime expression that produces a `u64` limit. The `Config` branch
///    includes a `#[allow(unused_parens)]` suppression because the generated
///    `match` arm wraps the resolution logic in an expression block that
///    needs to return a `u64` through a parenthesised path.
/// 4. **Generate the cost-measurement code** — embeds a call to
///    `env.cost_estimate().budget().cpu_instruction_cost()` (for CPU) or
///    `.memory_bytes_cost()` (for memory) and an `assert!` that the measured
///    value is strictly less than the limit.
/// 5. **Replace the function body** — wraps the original statements in a new
///    block that includes the limit-resolution helpers, the original
///    statements, the cost measurement, and the assertion check.
///
/// # Parameters
///
/// * `attr` — the token stream of the attribute arguments (e.g. `500_000` or
///   `env = "VAR"`).
/// * `item` — the token stream of the annotated function.
/// * `metric` — which Soroban budget metric to assert against.
///
/// # Returns
///
/// A [`proc_macro::TokenStream`] containing the modified function with the
/// budget assertion appended.
fn generate_budget_assert(
    attr: TokenStream,
    item: TokenStream,
    metric: BudgetMetric,
) -> TokenStream {
    let attr_tokens: proc_macro2::TokenStream = attr.into();
    let item_tokens: proc_macro2::TokenStream = item.into();

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

/// Outcome of resolving a config value from `budget.json` at compile time.
enum ConfigResolution {
    /// Value found and parsed successfully.
    Value(u64),
    /// `budget.json` does not exist — caller should fall back to `u64::MAX`
    /// for backward compatibility.
    MissingFile,
    /// File exists but could not be parsed as valid JSON.
    MalformedJson,
    /// File exists, is valid JSON, but the requested key was not found.
    KeyNotFound,
}

/// Resolve a config value from `budget.json` at compile time using serde_json.
fn resolve_config_value(key: &str) -> ConfigResolution {
    let path = std::path::Path::new("budget.json");
    let content = match std::fs::read_to_string(path) {
        Ok(file_contents) => file_contents,
        Err(_) => return ConfigResolution::MissingFile,
    };
    let parsed: serde_json::Value = match serde_json::from_str(&content) {
        Ok(parsed_json) => parsed_json,
        Err(_) => return ConfigResolution::MalformedJson,
    };
    match parsed.get(key).and_then(|json_value| json_value.as_u64()) {
        Some(numeric_value) => ConfigResolution::Value(numeric_value),
        None => ConfigResolution::KeyNotFound,
    }
}

fn generate_limit_expr(limit: &BudgetLimit, metric_label: &str) -> proc_macro2::TokenStream {
    match limit {
        BudgetLimit::Int(int_limit) => quote! { #int_limit },
        BudgetLimit::EnvVar(env_var_name) => quote! {
            budget_env_resolve(#env_var_name)
                .map(|s| s.parse::<u64>().unwrap_or_else(|_| {
                    panic!(
                        "{}: env var {}={:?} is not a valid u64",
                        #metric_label,
                        #env_var_name,
                        s
                    )
                }))
                .unwrap_or(u64::MAX)
        },
        BudgetLimit::Config(key) => {
            // Try to resolve the config value at compile time using serde_json.
            // When `budget.json` exists during compilation, the value is injected
            // directly as a literal — zero runtime overhead (O(1) HashMap lookup
            // done once during macro expansion).
            match resolve_config_value(key) {
                ConfigResolution::Value(numeric_value) => quote! { #numeric_value },
                // Fall back to runtime resolution when `budget.json` is not
                // available at compile time (e.g. tests that create the file
                // dynamically). Uses std-only code for maximum compatibility.
                _ => quote! {
                    {
                        let path = ::std::path::Path::new("budget.json");
                        match ::std::fs::read_to_string(path) {
                            Ok(content) => {
                                let config_map: ::std::collections::HashMap<String, u64> = {
                                    let mut map = ::std::collections::HashMap::new();
                                    let bytes = content.as_bytes();
                                    let mut byte_index = 0;
                                    while byte_index < bytes.len() {
                                        match bytes[byte_index] {
                                            b'{' | b',' | b' ' | b'\n' | b'\t' | b'\r' => {
                                                byte_index += 1;
                                            }
                                            b'}' => break,
                                            b'"' => {
                                                byte_index += 1;
                                                let key_start = byte_index;
                                                while byte_index < bytes.len() && bytes[byte_index] != b'"' {
                                                    byte_index += 1;
                                                }
                                                let key = ::std::string::String::from_utf8_lossy(
                                                    &bytes[key_start..byte_index]
                                                ).into_owned();
                                                byte_index += 1;
                                                while byte_index < bytes.len()
                                                    && (bytes[byte_index] == b':' || bytes[byte_index] == b' '
                                                        || bytes[byte_index] == b'\n' || bytes[byte_index] == b'\t')
                                                {
                                                    byte_index += 1;
                                                }
                                                let val_start = byte_index;
                                                while byte_index < bytes.len() && bytes[byte_index].is_ascii_digit() {
                                                    byte_index += 1;
                                                }
                                                if val_start < byte_index {
                                                    if let Ok(parsed_value) = ::std::string::String::from_utf8_lossy(
                                                        &bytes[val_start..byte_index]
                                                    ).parse::<u64>() {
                                                        map.insert(key, parsed_value);
                                                    }
                                                }
                                            }
                                            _ => { byte_index += 1; }
                                        }
                                    }
                                    map
                                };
                                match config_map.get(#key).copied() {
                                    Some(v) => v,
                                    None => ::std::panic!(
                                        "{}: key '{}' not found or invalid in budget.json",
                                        #metric_label,
                                        #key,
                                    ),
                                }
                            }
                            Err(_) => u64::MAX,
                        }
                    }
                },
            }
        }
    }
}

fn generate_budget_assert(spec: BudgetSpec, item: TokenStream) -> TokenStream {
    let mut input_fn = match syn::parse2::<ItemFn>(item.into()) {
        Ok(parsed_fn) => parsed_fn,
        Err(parse_error) => return TokenStream::from(parse_error.to_compile_error()),
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
        Err(parse_error) => return TokenStream::from(parse_error.to_compile_error()),
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
        Ok(parsed_limit) => parsed_limit,
        Err(parse_error) => return TokenStream::from(parse_error.to_compile_error()),
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

    let limit_expr = generate_limit_expr(&limit, "budget_write_bytes_lt");

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
        Ok(parsed_limit) => parsed_limit,
        Err(parse_error) => return TokenStream::from(parse_error.to_compile_error()),
    };
    let mut input_fn = match syn::parse::<ItemFn>(item) {
        Ok(parsed_fn) => parsed_fn,
        Err(parse_error) => return TokenStream::from(parse_error.to_compile_error()),
    };

    let stmts = &input_fn.block.stmts;

    let limit_expr = match limit {
        BudgetLimit::Int(int_limit) => quote! { #int_limit },
        BudgetLimit::EnvVar(env_var_name) => quote! {
            std::env::var(#env_var_name)
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
        Ok(parsed_limit) => parsed_limit,
        Err(parse_error) => return TokenStream::from(parse_error.to_compile_error()),
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
        Ok(parsed_spec) => parsed_spec,
        Err(parse_error) => return TokenStream::from(parse_error.to_compile_error()),
    };
    generate_budget_assert(spec, item)
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
                format!(
                    "unknown growth model `{other}`, expected `linear` or `quadratic`"
                ),
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
