extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn, LitInt};

/// Asserts that the CPU instructions used by `env` are less than N.
/// Must be placed on a test function that has a local `env` variable.
#[proc_macro_attribute]
pub fn budget_cpu_lt(attr: TokenStream, item: TokenStream) -> TokenStream {
    let limit = parse_macro_input!(attr as LitInt);
    let mut input_fn = parse_macro_input!(item as ItemFn);

    let stmts = &input_fn.block.stmts;
    let limit_u64: u64 = limit.base10_parse().unwrap();

    let env_ident = proc_macro2::Ident::new("env", proc_macro2::Span::call_site());

    let new_block = quote! {
        {
            #(#stmts)*

            let budget = #env_ident.cost_estimate().budget();
            let cpu_cost = budget.cpu_instruction_cost();
            assert!(
                cpu_cost < #limit_u64,
                "CPU instruction cost {} exceeded limit {} - local estimate, underestimates real network cost",
                cpu_cost,
                #limit_u64
            );
        }
    };

    *input_fn.block = syn::parse2(new_block).unwrap();

    TokenStream::from(quote! {
        #input_fn
    })
}

/// Asserts that the memory bytes used by `env` are less than N.
/// Must be placed on a test function that has a local `env` variable.
#[proc_macro_attribute]
pub fn budget_mem_lt(attr: TokenStream, item: TokenStream) -> TokenStream {
    let limit = parse_macro_input!(attr as LitInt);
    let mut input_fn = parse_macro_input!(item as ItemFn);

    let stmts = &input_fn.block.stmts;
    let limit_u64: u64 = limit.base10_parse().unwrap();

    let env_ident = proc_macro2::Ident::new("env", proc_macro2::Span::call_site());

    let new_block = quote! {
        {
            #(#stmts)*

            let budget = #env_ident.cost_estimate().budget();
            let mem_cost = budget.memory_bytes_cost();
            assert!(
                mem_cost < #limit_u64,
                "Memory bytes cost {} exceeded limit {} - local estimate, underestimates real network cost",
                mem_cost,
                #limit_u64
            );
        }
    };

    *input_fn.block = syn::parse2(new_block).unwrap();

    TokenStream::from(quote! {
        #input_fn
    })
}
