extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2, TokenTree};
use quote::quote;
use syn::visit_mut::{self, VisitMut};
use syn::{
    parse::Parse, parse::ParseStream, parse_quote, Expr, Ident, Item, ItemFn, LitInt, LitStr,
    Macro, Stmt, Token,
};

#[derive(Clone)]
enum BudgetLimit {
    Int(u64),
    EnvVar(String),
    Config(String),
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
/// - `env = "VAR_NAME"` to read the limit from an environment variable.
/// - `config = "key"` to read the limit from a `budget.json` file.
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
        BudgetLimit::Config(key) => {
            // Try to resolve the config value at compile time using serde_json.
            // When `budget.json` exists during compilation, the value is injected
            // directly as a literal — zero runtime overhead (O(1) HashMap lookup
            // done once during macro expansion).
            match resolve_config_value(key) {
                ConfigResolution::Value(n) => quote! { #n },
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
                                    let mut i = 0;
                                    while i < bytes.len() {
                                        match bytes[i] {
                                            b'{' | b',' | b' ' | b'\n' | b'\t' | b'\r' => {
                                                i += 1;
                                            }
                                            b'}' => break,
                                            b'"' => {
                                                i += 1;
                                                let key_start = i;
                                                while i < bytes.len() && bytes[i] != b'"' {
                                                    i += 1;
                                                }
                                                let key = ::std::string::String::from_utf8_lossy(
                                                    &bytes[key_start..i]
                                                ).into_owned();
                                                i += 1;
                                                while i < bytes.len()
                                                    && (bytes[i] == b':' || bytes[i] == b' '
                                                        || bytes[i] == b'\n' || bytes[i] == b'\t')
                                                {
                                                    i += 1;
                                                }
                                                let val_start = i;
                                                while i < bytes.len() && bytes[i].is_ascii_digit() {
                                                    i += 1;
                                                }
                                                if val_start < i {
                                                    if let Ok(n) = ::std::string::String::from_utf8_lossy(
                                                        &bytes[val_start..i]
                                                    ).parse::<u64>() {
                                                        map.insert(key, n);
                                                    }
                                                }
                                            }
                                            _ => { i += 1; }
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
    assertion: TokenStream2,
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

    fn visit_item_mut(&mut self, _item: &mut Item) {
        // Nested items (e.g. helper `fn`s declared in the body) have their own returns.
    }

    fn visit_macro_mut(&mut self, mac: &mut Macro) {
        if let Some(span) = find_return_token(mac.tokens.clone()) {
            self.macro_returns.push(span);
        }
    }
}

/// Finds a `return` token anywhere in a macro's token stream.
fn find_return_token(tokens: TokenStream2) -> Option<Span> {
    for token in tokens {
        match token {
            TokenTree::Ident(ident) if ident == "return" => return Some(ident.span()),
            TokenTree::Group(group) => {
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
    prelude: TokenStream2,
    assertion: TokenStream2,
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
        Some(Stmt::Expr(expr, None)) if !matches!(expr, Expr::Return(_)) => {
            match input_fn.block.stmts.pop() {
                Some(Stmt::Expr(expr, None)) => Some(expr),
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
        Err(e) => return Some(TokenStream::from(e.to_compile_error())),
    };
    // A body ending in `return`/`panic!` makes the trailing assertion unreachable;
    // the paths that do reach an exit already asserted.
    input_fn
        .attrs
        .push(parse_quote!(#[allow(unreachable_code)]));

    None
}

fn generate_budget_assert(spec: BudgetSpec, item: TokenStream) -> TokenStream {
    let mut input_fn = match syn::parse2::<ItemFn>(item.into()) {
        Ok(f) => f,
        Err(e) => return TokenStream::from(e.to_compile_error()),
    };

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
/// The macro injects an assertion check that measures
/// `env.cost_estimate().budget().cpu_instruction_cost()` on every path that leaves the test
/// function, so bodies ending in a tail expression (`fn test() -> Result<(), Error>`)
/// and bodies with early `return`s are both supported. A `?` that propagates an error
/// skips the check, but the test fails on that error anyway.
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
///
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

    // Same exit-path instrumentation as the other budget macros.
    let assertion = quote! {
        {
            #(#stmts)*

            // Wrap injected temporaries in their own scope so they never
            // collide with user-declared `budget`, `write_bytes_cost`,
            // or `limit_u64` names in the test function body.
            {
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

    if let Some(tokens) = instrument_exit_paths(&mut input_fn, quote! {}, assertion) {
        return tokens;
    }

    TokenStream::from(quote! {
        #input_fn
    })
}
/// Asserts that the memory bytes used by `env` are strictly less than a specified limit.
///
/// Must be placed on a test function that contains a local `env` variable (a `soroban_sdk::Env`).
/// The macro injects an assertion check that measures
/// `env.cost_estimate().budget().memory_bytes_cost()` on every path that leaves the test
/// function, so bodies ending in a tail expression (`fn test() -> Result<(), Error>`)
/// and bodies with early `return`s are both supported. A `?` that propagates an error
/// skips the check, but the test fails on that error anyway.
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
/// Limits can be specified as `cpu = N` and `mem = M`.
///
/// This checks a *local* estimate. Real network cost can differ from it
/// significantly in either direction depending on the build profile — see
/// `docs/src/mechanics.md` for measurements. Use `cargo budget-report` for
/// network ground truth.
#[proc_macro_attribute]
pub fn budget_lt(attr: TokenStream, item: TokenStream) -> TokenStream {
    let spec = match syn::parse2::<BudgetSpec>(attr.into()) {
        Ok(s) => s,
        Err(e) => return TokenStream::from(e.to_compile_error()),
    };
    generate_budget_assert(spec, item)
}
