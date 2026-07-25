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

/// Asserts that the CPU instructions used by `env` are less than N.
/// Must be placed on a test function that has a local `env` variable.
///
/// This checks a *local* estimate. Real network cost can differ from it
/// significantly in either direction depending on the build profile — see
/// `docs/src/mechanics.md` for measurements. Use `cargo budget-report` for
/// network ground truth.
///
/// When using `env = "VAR"`, an unset environment variable means "no limit"
/// (the assertion will always pass). The test will panic if the variable is
/// set but its value cannot be parsed as a `u64`.
#[proc_macro_attribute]
pub fn budget_cpu_lt(attr: TokenStream, item: TokenStream) -> TokenStream {
    generate_budget_assert(attr, item, BudgetMetric::CpuInstructionCost)
}

/// Asserts that the memory bytes used by `env` are less than N.
/// Must be placed on a test function that has a local `env` variable.
///
/// This checks a *local* estimate. Real network cost can differ from it
/// significantly in either direction depending on the build profile — see
/// `docs/src/mechanics.md` for measurements. Use `cargo budget-report` for
/// network ground truth.
///
/// When using `env = "VAR"`, an unset environment variable means "no limit"
/// (the assertion will always pass). The test will panic if the variable is
/// set but its value cannot be parsed as a `u64`.
#[proc_macro_attribute]
pub fn budget_mem_lt(attr: TokenStream, item: TokenStream) -> TokenStream {
    generate_budget_assert(attr, item, BudgetMetric::MemoryBytesCost)
}
