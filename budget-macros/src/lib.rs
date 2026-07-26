extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse::Parse, parse::ParseStream, Ident, ItemFn, LitInt, LitStr, Token};

enum BudgetLimit {
    Int(u64),
    EnvVar(String),
    Config(String),
    // TODO: Add support for parsing a default value if the env var is missing
}

enum BudgetMetric {
    CpuInstructionCost,
    MemoryBytesCost,
}

impl Parse for BudgetLimit {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(Ident) {
            let ident: Ident = input.parse()?;
            input.parse::<Token![=]>()?;
            let lit: LitStr = input.parse()?;
            match ident.to_string().as_str() {
                "env" => Ok(BudgetLimit::EnvVar(lit.value())),
                "config" => Ok(BudgetLimit::Config(lit.value())),
                other => Err(syn::Error::new(
                    ident.span(),
                    format!("expected `env` or `config`, got `{}`", other),
                )),
            }
        } else {
            let lit: LitInt = input.parse()?;
            Ok(BudgetLimit::Int(lit.base10_parse()?))
        }
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
    /// File exists, is valid JSON, but the requested key was not found or
    /// its value is not a valid u64.
    KeyNotFound,
}

/// Resolve a config value from `budget.json` at compile time using serde_json.
///
/// This replaces the previous runtime linear scan (`content.find()`) with a
/// single O(1) HashMap lookup performed during macro expansion.
fn resolve_config_value(key: &str) -> ConfigResolution {
    let path = std::path::Path::new("budget.json");
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return ConfigResolution::MissingFile,
    };
    let parsed: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return ConfigResolution::MalformedJson,
    };
    match parsed.get(key).and_then(|v| v.as_u64()) {
        Some(n) => ConfigResolution::Value(n),
        None => ConfigResolution::KeyNotFound,
    }
}

fn generate_budget_assert(
    attr: TokenStream,
    item: TokenStream,
    metric: BudgetMetric,
) -> TokenStream {
    let attr_tokens: proc_macro2::TokenStream = attr.into();
    let item_tokens: proc_macro2::TokenStream = item.into();

    let limit = match syn::parse2::<BudgetLimit>(attr_tokens.clone()) {
        Ok(l) => l,
        Err(e) => return TokenStream::from(e.into_compile_error()),
    };
    let mut input_fn = match syn::parse2::<ItemFn>(item_tokens) {
        Ok(f) => f,
        Err(e) => return TokenStream::from(e.into_compile_error()),
    };

    let stmts = &input_fn.block.stmts;

    let metric_label = match &metric {
        BudgetMetric::CpuInstructionCost => "budget_cpu_lt",
        BudgetMetric::MemoryBytesCost => "budget_mem_lt",
    };

    let limit_expr = match limit {
        BudgetLimit::Int(n) => quote! { #n },
        BudgetLimit::EnvVar(var) => quote! {
            match budget_env_resolve(#var) {
                Some(s) => s.parse::<u64>().unwrap_or_else(|_| {
                    panic!(
                        "{}: env var {}={:?} is not a valid u64",
                        #metric_label,
                        #var,
                        s
                    )
                }),
                None => u64::MAX,
            }
        },
        BudgetLimit::Config(key) => {
            // Resolve the config value at compile time using serde_json.
            // This replaces the previous runtime linear scan with a single
            // O(1) HashMap lookup, performed during macro expansion.
            match resolve_config_value(&key) {
                ConfigResolution::Value(n) => quote! { #n },
                // Preserve backward compatibility: missing file → u64::MAX (no limit).
                ConfigResolution::MissingFile => quote! { u64::MAX },
                // File exists but is malformed → panic with descriptive message.
                ConfigResolution::MalformedJson => quote! {
                    {
                        panic!(
                            "{}: budget.json is malformed and could not be parsed",
                            #metric_label,
                        )
                    }
                },
                // File exists, valid JSON, but key not found → panic with descriptive message.
                ConfigResolution::KeyNotFound => quote! {
                    {
                        panic!(
                            "{}: key '{}' not found in budget.json",
                            #metric_label,
                            #key,
                        )
                    }
                },
            }
        }
    };

    // The `Config` variant is now resolved at compile time, so the inline
    // `parse_config_value` closure was removed. `budget_env_resolve` is
    // still needed for the `EnvVar` variant below.

    let env_ident = proc_macro2::Ident::new("env", proc_macro2::Span::call_site());

    let (cost_ident, cost_expr, assert_msg) = match metric {
        BudgetMetric::CpuInstructionCost => (
            proc_macro2::Ident::new("cpu_cost", proc_macro2::Span::call_site()),
            quote! { budget.cpu_instruction_cost() },
            "CPU instruction cost {} exceeded limit {} - local estimate, real network cost may differ significantly in either direction",
        ),
        BudgetMetric::MemoryBytesCost => (
            proc_macro2::Ident::new("mem_cost", proc_macro2::Span::call_site()),
            quote! { budget.memory_bytes_cost() },
            "Memory bytes cost {} exceeded limit {} - local estimate, real network cost may differ significantly in either direction",
        ),
    };

    let new_block = quote! {
        {
            #[allow(unused_variables)]
            let budget_env_resolve = |var: &str| -> Option<String> {
                std::env::var(var).ok()
            };

            #(#stmts)*

            let budget = #env_ident.cost_estimate().budget();
            let #cost_ident = #cost_expr;
            let limit_u64: u64 = #limit_expr;
            assert!(
                #cost_ident < limit_u64,
                #assert_msg,
                #cost_ident,
                limit_u64
            );
        }
    };

    *input_fn.block = syn::parse2(new_block).unwrap();

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
#[proc_macro_attribute]
pub fn budget_cpu_lt(attr: TokenStream, item: TokenStream) -> TokenStream {
    generate_budget_assert(attr, item, BudgetMetric::CpuInstructionCost)
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
#[proc_macro_attribute]
pub fn budget_mem_lt(attr: TokenStream, item: TokenStream) -> TokenStream {
    generate_budget_assert(attr, item, BudgetMetric::MemoryBytesCost)
}
