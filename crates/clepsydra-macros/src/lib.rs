use proc_macro_crate::{FoundCrate, crate_name};
use quote::{format_ident, quote};
use std::collections::HashMap;
use std::fs;
use syn::parse::{Parse, ParseStream};
use syn::{Ident, LitStr, Result, Token, parse_macro_input};

use clepsydra_core::ast::Expr;
use clepsydra_core::config::Config;
use clepsydra_core::new_topology;
use clepsydra_core::symbolic::SymbolicState;
use clepsydra_core::topology::Topology;

fn eval_error_path() -> syn::Result<proc_macro2::TokenStream> {
    let path = crate_name("clepsydra-lib")
        .map(|found| match found {
            FoundCrate::Itself => quote! { crate::clepsydra_core::errors::EvalError },
            FoundCrate::Name(name) => {
                let crate_ident = format_ident!("{name}");
                quote! { ::#crate_ident::clepsydra_core::errors::EvalError }
            }
        })
        .or_else(|_| {
            crate_name("clepsydra-core").map(|found| match found {
                FoundCrate::Itself => quote! { crate::errors::EvalError },
                FoundCrate::Name(name) => {
                    let crate_ident = format_ident!("{name}");
                    quote! { ::#crate_ident::errors::EvalError }
                }
            })
        })
        .map_err(|error| {
            syn::Error::new(
                proc_macro2::Span::call_site(),
                format!("Unable to locate clepsydra error types: {error}"),
            )
        })?;
    Ok(path)
}

fn emit_eval(topology: &Topology<'_>) -> syn::Result<proc_macro2::TokenStream> {
    let symbolic_state = SymbolicState::new(topology);
    let node_slots: HashMap<_, _> = topology
        .graph()
        .iter()
        .enumerate()
        .map(|(slot, (id, _))| (id, slot))
        .collect();
    let node_index: HashMap<_, _> = symbolic_state
        .ast()
        .iter()
        .enumerate()
        .map(|(index, (id, _))| (id, index))
        .collect();

    macro_rules! get_index {
        ($expr_id:expr) => {{
            let expr_id = $expr_id;
            node_index.get(&expr_id).copied().ok_or_else(|| {
                syn::Error::new(
                    proc_macro2::Span::call_site(),
                    "expression ID missing from AST index",
                )
            })?
        }};
    }

    let statements: Vec<_> = symbolic_state
        .ast()
        .iter()
        .map(|(id, expr)| -> syn::Result<_> {
            let expr_id = format_ident!("expr_{}", get_index!(id));
            let statement = match expr {
                Expr::Var(variable) => {
                    let slot = node_slots.get(variable).copied().ok_or_else(|| {
                        syn::Error::new(
                            proc_macro2::Span::call_site(),
                            "AST variable has no associated positional slot",
                        )
                    })?;
                    quote! { let #expr_id = state[#slot]; }
                }
                Expr::Const(value) => quote! { let #expr_id = #value; },
                Expr::Add(lhs, rhs) => {
                    let lhs_id = format_ident!("expr_{}", get_index!(*lhs));
                    let rhs_id = format_ident!("expr_{}", get_index!(*rhs));
                    quote! { let #expr_id = #lhs_id.saturating_add(#rhs_id); }
                }
                Expr::SatSub(lhs, rhs) => {
                    let lhs_id = format_ident!("expr_{}", get_index!(*lhs));
                    let rhs_id = format_ident!("expr_{}", get_index!(*rhs));
                    quote! { let #expr_id = #lhs_id.saturating_sub(#rhs_id); }
                }
            };
            Ok(statement)
        })
        .collect::<syn::Result<_>>()?;

    let node_statements: Vec<_> = symbolic_state
        .node_exprs()
        .iter()
        .map(|(node_id, expr_id)| {
            let slot = node_slots.get(node_id).copied().ok_or_else(|| {
                syn::Error::new(
                    proc_macro2::Span::call_site(),
                    "node expression has no associated positional slot",
                )
            })?;
            let expr = format_ident!("expr_{}", get_index!(*expr_id));
            Ok(quote! { state[#slot] = #expr; })
        })
        .collect::<syn::Result<_>>()?;

    let n = symbolic_state.node_count();

    Ok(quote! {
        |mut state: [u64; #n]| -> [u64; #n] {
            #(#statements)*
            #(#node_statements)*
            state
        }
    })
}

fn emit_named_eval(topology: &Topology<'_>) -> syn::Result<proc_macro2::TokenStream> {
    let error_path = eval_error_path()?;
    let names: Vec<_> = topology
        .graph()
        .iter()
        .map(|(id, _)| {
            let name = topology.node_name(id);
            quote! { #name }
        })
        .collect();
    let n = topology.node_count();
    let ordered_eval = emit_eval(topology)?;
    let name_matches: Vec<_> = names
        .iter()
        .enumerate()
        .map(|(slot, name)| {
            quote! { #name => Some(#slot), }
        })
        .collect();

    let closure = quote! {
        move |state| {
            let mut values = [0_u64; #n];
            let mut seen = [false; #n];

            for (name, value) in state.iter().copied() {
                let Some(slot) = (match name {
                    #(#name_matches)*
                    _ => None,
                }) else {
                    return Err(#error_path::UnknownInput {
                        name: name.to_owned(),
                    });
                };
                if seen[slot] {
                    return Err(#error_path::DuplicateInput {
                        name: name.to_owned(),
                    });
                }
                seen[slot] = true;
                values[slot] = value;
            }

            if let Some((slot, _)) = seen.iter().enumerate().find(|(_, seen)| !**seen) {
                return Err(#error_path::MissingInput {
                    name: [#(#names),*][slot].to_owned(),
                });
            }

            let values = (#ordered_eval)(values);
            Ok(::std::array::from_fn(|index| {
                let (name, _) = state[index];
                let Some(slot) = (match name {
                    #(#name_matches)*
                    _ => None,
                }) else {
                    unreachable!()
                };
                (name, values[slot])
            }))
        }
    };

    Ok(quote! {
        {
            let eval: for<'input> fn(
                [(&'input str, u64); #n]
            ) -> ::std::result::Result<[
                (&'input str, u64); #n
            ], #error_path> = #closure;
            eval
        }
    })
}

fn emit_vec_eval(topology: &Topology<'_>) -> syn::Result<proc_macro2::TokenStream> {
    let error_path = eval_error_path()?;
    let names: Vec<_> = topology
        .graph()
        .iter()
        .map(|(id, _)| {
            let name = topology.node_name(id);
            quote! { #name }
        })
        .collect();
    let n = topology.node_count();
    let ordered_eval = emit_eval(topology)?;
    let name_matches: Vec<_> = names
        .iter()
        .enumerate()
        .map(|(slot, name)| quote! { #name => Some(#slot), })
        .collect();

    Ok(quote! {
        {
            let eval: for<'input> fn(
                &'input [(&'input str, u64)]
            ) -> ::std::result::Result<Vec<(&'input str, u64)>, #error_path> = |state| {
                if state.len() != #n {
                    return Err(#error_path::InvalidInputCount {
                        expected: #n,
                        actual: state.len(),
                    });
                }
                let mut values = [0_u64; #n];
                let mut seen = [false; #n];
                for (name, value) in state.iter().copied() {
                    let Some(slot) = (match name {
                        #(#name_matches)*
                        _ => None,
                    }) else {
                        return Err(#error_path::UnknownInput {
                            name: name.to_owned(),
                        });
                    };
                    if seen[slot] {
                        return Err(#error_path::DuplicateInput {
                            name: name.to_owned(),
                        });
                    }
                    seen[slot] = true;
                    values[slot] = value;
                }
                let values = (#ordered_eval)(values);
                state.iter().map(|(name, _)| {
                    let Some(slot) = (match *name {
                        #(#name_matches)*
                        _ => None,
                    }) else {
                        unreachable!()
                    };
                    Ok((*name, values[slot]))
                }).collect()
            };
            eval
        }
    })
}

fn emit_map_eval(topology: &Topology<'_>) -> syn::Result<proc_macro2::TokenStream> {
    let error_path = eval_error_path()?;
    let names: Vec<_> = topology
        .graph()
        .iter()
        .map(|(id, _)| {
            let name = topology.node_name(id);
            quote! { #name }
        })
        .collect();
    let n = topology.node_count();
    let ordered_eval = emit_eval(topology)?;
    let name_matches: Vec<_> = names
        .iter()
        .enumerate()
        .map(|(slot, name)| quote! { #name => Some(#slot), })
        .collect();
    let output_statements: Vec<_> = names
        .iter()
        .enumerate()
        .map(|(slot, name)| quote! { result.insert((#name).to_owned(), values[#slot]); })
        .collect();
    let generated_names = quote! { [#(#names),*] };

    let closure = quote! {
        move |state: ::std::collections::HashMap<::std::string::String, u64>| {
            let mut values = [0_u64; #n];
            let mut seen = [false; #n];

            for (name, value) in state {
                let Some(slot) = (match name.as_str() {
                    #(#name_matches)*
                    _ => None,
                }) else {
                    return Err(#error_path::UnknownInput { name });
                };
                if seen[slot] {
                    return Err(#error_path::DuplicateInput { name });
                }
                seen[slot] = true;
                values[slot] = value;
            }

            if let Some((slot, _)) = seen.iter().enumerate().find(|(_, seen)| !**seen) {
                return Err(#error_path::MissingInput {
                    name: #generated_names[slot].to_owned(),
                });
            }

            let values = (#ordered_eval)(values);
            let mut result = ::std::collections::HashMap::with_capacity(#n);
            #(#output_statements)*
            Ok(result)
        }
    };

    Ok(quote! {
        {
            let eval: fn(
                ::std::collections::HashMap<::std::string::String, u64>,
            ) -> ::std::result::Result<
                ::std::collections::HashMap<::std::string::String, u64>,
                #error_path,
            > = #closure;
            eval
        }
    })
}

struct Input {
    name: Ident,
    config: Config,
}

fn parse_config(source: &str, format: Option<&str>) -> Result<Config> {
    let parse_error = |error: String| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            format!("Failed to parse configuration: {error}"),
        )
    };

    match format {
        Some("toml") => toml::from_str(source).map_err(|error| parse_error(error.to_string())),
        Some("json") => {
            serde_json::from_str(source).map_err(|error| parse_error(error.to_string()))
        }
        Some("yaml" | "yml") => {
            serde_yaml::from_str(source).map_err(|error| parse_error(error.to_string()))
        }
        Some(extension) => Err(parse_error(format!(
            "unsupported file extension '.{extension}'; use .toml, .json, .yaml, or .yml"
        ))),
        None => toml::from_str::<Config>(source).map_or_else(
            |_| {
                serde_json::from_str::<Config>(source).map_or_else(
                    |_| {
                        serde_yaml::from_str::<Config>(source)
                            .map_err(|error| parse_error(error.to_string()))
                    },
                    Ok,
                )
            },
            Ok,
        ),
    }
}

impl Parse for Input {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut name = format_ident!("Evaluator");
        let mut source_str = String::from("clepsydra.toml");

        if input.is_empty() {
        } else if input.peek(syn::Ident) {
            name = input.parse::<Ident>()?;
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
                let lit: LitStr = input.parse()?;
                source_str = lit.value();
            }
        } else if input.peek(syn::LitStr) {
            let lit: LitStr = input.parse()?;
            source_str = lit.value();
        } else {
            return Err(
                input.error("Expected a struct identifier or a TOML, JSON, or YAML string literal")
            );
        }

        let trimmed = source_str.trim();
        let path = std::path::Path::new(trimmed);
        let extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase);
        let is_config_path = !trimmed.contains(['\n', '\r'])
            && extension
                .as_deref()
                .is_some_and(|extension| matches!(extension, "toml" | "json" | "yaml" | "yml"));
        let config_str = if is_config_path {
            let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into());
            let path = std::path::Path::new(&manifest_dir).join(trimmed);
            fs::read_to_string(&path).map_err(|e| {
                syn::Error::new(
                    proc_macro2::Span::call_site(),
                    format!("Failed to read config file {}: {e}", path.display()),
                )
            })?
        } else {
            trimmed.to_string()
        };

        let format = is_config_path.then_some(extension).flatten();
        let config = parse_config(&config_str, format.as_deref())?;

        Ok(Self { name, config })
    }
}

#[proc_macro]
pub fn clepsydra_eval(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let Input { name: _, config } = parse_macro_input!(input as Input);
    let result = (|| -> syn::Result<_> {
        let topology = new_topology!(&config)
            .map_err(|error| syn::Error::new(proc_macro2::Span::call_site(), error.to_string()))?;
        emit_named_eval(&topology)
    })();

    match result {
        Ok(tokens) => tokens.into(),
        Err(error) => error.into_compile_error().into(),
    }
}

#[proc_macro]
pub fn clepsydra_map_eval(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let Input { name: _, config } = parse_macro_input!(input as Input);
    let result = (|| -> syn::Result<_> {
        let topology = new_topology!(&config)
            .map_err(|error| syn::Error::new(proc_macro2::Span::call_site(), error.to_string()))?;
        emit_map_eval(&topology)
    })();

    match result {
        Ok(tokens) => tokens.into(),
        Err(error) => error.into_compile_error().into(),
    }
}

#[proc_macro]
pub fn clepsydra_vec_eval(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let Input { name: _, config } = parse_macro_input!(input as Input);
    let result = (|| -> syn::Result<_> {
        let topology = new_topology!(&config)
            .map_err(|error| syn::Error::new(proc_macro2::Span::call_site(), error.to_string()))?;
        emit_vec_eval(&topology)
    })();

    match result {
        Ok(tokens) => tokens.into(),
        Err(error) => error.into_compile_error().into(),
    }
}

#[proc_macro]
pub fn clepsydra(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let Input { name, config } = parse_macro_input!(input as Input);
    let result = (|| -> syn::Result<_> {
        let topology = new_topology!(&config)
            .map_err(|error| syn::Error::new(proc_macro2::Span::call_site(), error.to_string()))?;
        let names: Vec<_> = topology
            .graph()
            .iter()
            .map(|(id, _)| {
                let name = topology.node_name(id);
                quote! { #name }
            })
            .collect();
        let n = topology.node_count();
        let error_path = eval_error_path()?;
        let ordered_eval = emit_eval(&topology)?;
        let named_eval = emit_named_eval(&topology)?;
        let vec_eval = emit_vec_eval(&topology)?;
        let map_eval = emit_map_eval(&topology)?;

        Ok(quote! {
            pub struct #name;

            impl #name {
                pub const NODE_COUNT: usize = #n;

                pub const NAMES: [&'static str; #n] = [
                    #(#names),*
                ];

                pub const ORDERED_EVAL: fn([u64; #n]) -> [u64; #n] = #ordered_eval;

                pub const EVAL: for<'input> fn(
                    [(&'input str, u64); #n]
                ) -> ::std::result::Result<[
                    (&'input str, u64); #n
                ], #error_path> = #named_eval;

                pub const VEC_EVAL: for<'input> fn(
                    &'input [(&'input str, u64)]
                ) -> ::std::result::Result<
                    Vec<(&'input str, u64)>,
                    #error_path,
                > = #vec_eval;

                pub const MAP_EVAL: fn(
                    ::std::collections::HashMap<::std::string::String, u64>,
                ) -> ::std::result::Result<
                    ::std::collections::HashMap<::std::string::String, u64>,
                    #error_path,
                > = #map_eval;

                pub fn ordered_eval(state: [u64; #n]) -> [u64; #n] {
                    Self::ORDERED_EVAL(state)
                }

                pub fn eval<'state>(
                    state: [(&'state str, u64); #n],
                ) -> ::std::result::Result<[
                    (&'state str, u64); #n
                ], #error_path> {
                    Self::EVAL(state)
                }

                pub fn vec_eval<'state>(
                    state: &'state [(&'state str, u64)],
                ) -> ::std::result::Result<Vec<(&'state str, u64)>, #error_path> {
                    Self::VEC_EVAL(state)
                }

                pub fn map_eval(
                    state: ::std::collections::HashMap<::std::string::String, u64>,
                ) -> ::std::result::Result<
                    ::std::collections::HashMap<::std::string::String, u64>,
                    #error_path,
                > {
                    Self::MAP_EVAL(state)
                }
            }
        })
    })();

    match result {
        Ok(tokens) => tokens.into(),
        Err(error) => error.into_compile_error().into(),
    }
}
