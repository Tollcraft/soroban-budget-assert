extern crate proc_macro;

use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{parse::Parse, parse::ParseStream, Expr, Ident, ItemFn, LitInt, LitStr, Token};

enum BudgetLimit {
    /// A literal integer limit provided directly in the attribute.
    Int(u64),
    EnvVar(String, Option<u64>),
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
                "env" => {
                    let raw = lit.value();
                    let (var, default) = match raw.find(':') {
                        Some(pos) => {
                            let var = raw[..pos].to_string();
                            let default_str = &raw[pos + 1..];
                            let default_val: u64 = default_str.parse().map_err(|e| {
                                syn::Error::new(
                                    lit.span(),
                                    format!("invalid default value after ':' in env string: {}", e),
                                )
                            })?;
                            (var, Some(default_val))
                        }
                        None => (raw, None),
                    };
                    Ok(BudgetLimit::EnvVar(var, default))
                }
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
        BudgetLimit::EnvVar(var, default) => {
            let fallback = match default {
                Some(d) => quote! { #d },
                None => quote! { u64::MAX },
            };
            quote! {
                match budget_env_resolve(#var) {
                    Some(s) => s.parse::<u64>().unwrap_or_else(|_| {
                        panic!(
                            "{}: env var {}={:?} is not a valid u64",
                            #metric_label,
                            #var,
                            s
                        )
                    }),
                    None => #fallback,
                }
            }
        }
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
