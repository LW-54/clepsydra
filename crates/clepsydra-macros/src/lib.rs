use quote::{format_ident, quote};
use std::collections::HashMap;
use std::fs;
use syn::parse::{Parse, ParseStream};
use syn::{Ident, LitStr, Result, Token, parse_macro_input};

use clepsydra_core::ast::Expr;
use clepsydra_core::config::Config;
use clepsydra_core::new_topology;
use clepsydra_core::symbolic::SymbolicState;

fn emit_eval(symbolic_state: &SymbolicState<'_>) -> syn::Result<proc_macro2::TokenStream> {
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
                    let name = symbolic_state.node_name(*variable).ok_or_else(|| {
                        syn::Error::new(
                            proc_macro2::Span::call_site(),
                            "AST variable has no associated node name",
                        )
                    })?;
                    quote! {
                        let #expr_id = match state.iter().find(|(n, _)| *n == #name) {
                            Some((_, value)) => *value,
                            None => return state,
                        };
                    }
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

    let mut node_exprs: Vec<_> = symbolic_state
        .node_exprs()
        .iter()
        .map(|(node_id, expr_id)| {
            let name = symbolic_state.node_name(*node_id).ok_or_else(|| {
                syn::Error::new(
                    proc_macro2::Span::call_site(),
                    "node expression has no associated node name",
                )
            })?;
            Ok((*node_id, *expr_id, name.to_owned()))
        })
        .collect::<syn::Result<_>>()?;
    node_exprs.sort_by(|left, right| left.2.cmp(&right.2));

    let node_statements: Vec<_> = node_exprs
        .iter()
        .map(|(_, expr_id, name)| {
            let expr = format_ident!("expr_{}", get_index!(*expr_id));
            Ok(quote! {
                if let Some(index) = state.iter().position(|(n, _)| *n == #name) {
                    state[index].1 = #expr;
                }
            })
        })
        .collect::<syn::Result<_>>()?;

    let n = symbolic_state.node_count();

    Ok(quote! {
        |mut state: [(&'static str, u64); #n]| -> [(&'static str, u64); #n] {
            #(#statements)*
            #(#node_statements)*
            state
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
        let mut name = format_ident!("Clepsydra");
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
pub fn clepsydra_closure(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let Input { name: _, config } = parse_macro_input!(input as Input);
    let result = (|| -> syn::Result<_> {
        let topology = new_topology!(&config)
            .map_err(|error| syn::Error::new(proc_macro2::Span::call_site(), error.to_string()))?;
        let eval = emit_eval(&SymbolicState::new(&topology))?;
        Ok(quote! { #eval })
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
                topology
                    .node_name(id)
                    .map(|name| quote! { #name })
                    .ok_or_else(|| {
                        syn::Error::new(
                            proc_macro2::Span::call_site(),
                            "node ID has no associated node name",
                        )
                    })
            })
            .collect::<syn::Result<_>>()?;
        let n = topology.node_count();
        let eval_fn = emit_eval(&SymbolicState::new(&topology))?;

        Ok(quote! {
            pub struct #name;

            impl #name {
                pub const NODE_COUNT: usize = #n;

                pub const NAMES: [&'static str; #n] = [
                    #(#names),*
                ];

                pub fn evaluate(
                    state: [(&'static str, u64); #n],
                ) -> [(&'static str, u64); #n] {
                    (#eval_fn)(state)
                }
            }
        })
    })();

    match result {
        Ok(tokens) => tokens.into(),
        Err(error) => error.into_compile_error().into(),
    }
}
