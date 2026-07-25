extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse::Parse, parse::ParseStream, Ident, ItemFn, LitInt, LitStr, Token};

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
        BudgetLimit::Config(key) => quote! {
            {
                let path = std::path::Path::new("budget.json");
                match std::fs::read_to_string(path) {
                    Ok(content) => {
                        #[allow(unused_parens)]
                        match parse_config_value(&content, #key) {
                            Some(v) => v,
                            None => {
                                panic!(
                                    "{}: key '{}' not found or invalid in budget.json",
                                    #metric_label,
                                    #key,
                                )
                            }
                        }
                    }
                    Err(_) => u64::MAX,
                }
            }
        },
    };

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
