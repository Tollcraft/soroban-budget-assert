extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse::Parse, parse::ParseStream, parse_macro_input, Ident, ItemFn, LitInt, LitStr, Token,
};

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

fn generate_budget_assert(attr: TokenStream, item: TokenStream, metric: BudgetMetric) -> TokenStream {
    let attr_tokens: proc_macro2::TokenStream = attr.into();
    let item_tokens: proc_macro2::TokenStream = item.into();

    let limit = syn::parse2::<BudgetLimit>(attr_tokens.clone()).unwrap();
    let mut input_fn = syn::parse2::<ItemFn>(item_tokens).unwrap();

    let stmts = &input_fn.block.stmts;

    let limit_expr = match limit {
        BudgetLimit::Int(n) => quote! { #n },
        BudgetLimit::EnvVar(var) => quote! {
            std::env::var(#var)
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(u64::MAX)
        },
    };

    let env_ident = proc_macro2::Ident::new("env", proc_macro2::Span::call_site());

    let (cost_ident, cost_expr, metric_label) = match metric {
        BudgetMetric::CpuInstructionCost => (
            proc_macro2::Ident::new("cpu_cost", proc_macro2::Span::call_site()),
            quote! { budget.cpu_instruction_cost() },
            "CPU instruction cost",
        ),
        BudgetMetric::MemoryBytesCost => (
            proc_macro2::Ident::new("mem_cost", proc_macro2::Span::call_site()),
            quote! { budget.memory_bytes_cost() },
            "Memory bytes cost",
        ),
    };

    let new_block = quote! {
        {
            #(#stmts)*

            let budget = #env_ident.cost_estimate().budget();
            let #cost_ident = #cost_expr;
            let limit_u64: u64 = #limit_expr;
            assert!(
                #cost_ident < limit_u64,
                "{} {} exceeded limit {} - local estimate, underestimates real network cost",
                #metric_label,
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
#[proc_macro_attribute]
pub fn budget_cpu_lt(attr: TokenStream, item: TokenStream) -> TokenStream {
    generate_budget_assert(attr, item, BudgetMetric::CpuInstructionCost)
}

/// Asserts that the memory bytes used by `env` are less than N.
/// Must be placed on a test function that has a local `env` variable.
#[proc_macro_attribute]
pub fn budget_mem_lt(attr: TokenStream, item: TokenStream) -> TokenStream {
    generate_budget_assert(attr, item, BudgetMetric::MemoryBytesCost)
}

