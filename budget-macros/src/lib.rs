extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse::Parse, parse::ParseStream, Ident, ItemFn, LitInt, LitStr, Token};

enum BudgetLimit {
    Int(u64),
    EnvVar(String),
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
            if ident != "env" {
                return Err(syn::Error::new(ident.span(), "expected `env`"));
            }
            input.parse::<Token![=]>()?;
            let lit: LitStr = input.parse()?;
            Ok(BudgetLimit::EnvVar(lit.value()))
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

    let limit = syn::parse2::<BudgetLimit>(attr_tokens.clone()).unwrap();
    let mut input_fn = syn::parse2::<ItemFn>(item_tokens).unwrap();

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

#[cfg(test)]
mod budget_limit_parser_proptest {
    use super::BudgetLimit;
    use proptest::prelude::*;
    use quote::quote;

    fn parse_budget_limit(tokens: proc_macro2::TokenStream) -> syn::Result<BudgetLimit> {
        syn::parse2(tokens)
    }

    proptest! {
        #[test]
        fn integer_attribute_parses_as_same_u64(n in any::<u64>()) {
            let parsed = parse_budget_limit(quote! { #n }).expect("integer limit should parse");
            match parsed {
                BudgetLimit::Int(value) => prop_assert_eq!(value, n),
                BudgetLimit::EnvVar(_) => prop_assert!(false, "expected integer limit"),
            }
        }

        #[test]
        fn env_attribute_parses_var_name(
            name in prop::string::string_regex(r"[A-Za-z_][A-Za-z0-9_]*").unwrap()
        ) {
            let lit = syn::LitStr::new(&name, proc_macro2::Span::call_site());
            let parsed = parse_budget_limit(quote! { env = #lit }).expect("env limit should parse");
            match parsed {
                BudgetLimit::EnvVar(var) => prop_assert_eq!(var, name),
                BudgetLimit::Int(_) => prop_assert!(false, "expected env limit"),
            }
        }

        #[test]
        fn non_env_ident_with_env_syntax_is_rejected(
            ident in prop::string::string_regex(r"[A-Za-z_][A-Za-z0-9_]*").unwrap()
        ) {
            prop_assume!(ident != "env");
            let id = prop_assume!(syn::parse_str::<syn::Ident>(&ident).ok());
            let lit = syn::LitStr::new("BUDGET_LIMIT", proc_macro2::Span::call_site());
            prop_assert!(parse_budget_limit(quote! { #id = #lit }).is_err());
        }
    }
}
