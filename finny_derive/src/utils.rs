use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{PatIdent, spanned::Spanned};

use crate::parse::{FsmState, FsmTransitionState};

pub fn remap_closure_inputs(inputs: &syn::punctuated::Punctuated<syn::Pat, syn::token::Comma>, access: &[TokenStream]) -> syn::Result<TokenStream> {
    if inputs.len() != access.len() {
        return Err(syn::Error::new(inputs.span(), &format!("Expected {} closure arguments, actually have {}.", access.len(), inputs.len())));
    }
    
    let input_remap: syn::Result<Vec<_>> = inputs.iter().enumerate().map(|(idx, input)| {
        match input {
            syn::Pat::Ident(PatIdent { ident, .. }) => {
                let rep = &access[idx];
                let q = quote! {
                    let #ident = #rep;
                };
                Ok(q)
            },
            syn::Pat::Wild(_) => Ok(TokenStream::new()),
            _ => { Err(syn::Error::new(input.span(), "Unsupported closure input.")) }
        }
    }).collect();
    let input_remap = input_remap?;
    
    let q = quote! {
        #(#input_remap)*
    };
    Ok(q)
}


pub fn get_closure(call: &syn::ExprMethodCall) -> syn::Result<&syn::ExprClosure> {
    match call.args.first() {
        Some(syn::Expr::Closure(closure)) => Ok(closure),
        _ => Err(syn::Error::new(call.span(), "Missing closure!"))
    }
}

pub fn strip_generics(mut ty: syn::Type) -> syn::Type {
    match ty {
        syn::Type::Path(ref mut tp) => {
            for mut seg in &mut tp.path.segments {
                seg.arguments = syn::PathArguments::None;
            }
        },
        _ => ()
    }

    ty
}

pub fn to_field_name(ty: &syn::Type) -> syn::Ident {
    let ty = strip_generics(ty.clone());

    let s = tokens_to_string(&ty);
    let snake = to_snake_case(&s);
    syn::Ident::new(&snake, ty.span())
}

pub fn tokens_to_string<T: quote::ToTokens>(t: &T) -> String {
    let mut tokens = TokenStream::new();
    t.to_tokens(&mut tokens);
    tokens.to_string()
}

// From rustc
pub fn to_snake_case(mut str: &str) -> String {
    let mut words = vec![];
    // Preserve leading underscores
    str = str.trim_start_matches(|c: char| {
        if c == '_' {
            words.push(String::new());
            true
        } else {
            false
        }
    });
    for s in str.split('_') {
        let mut last_upper = false;
        let mut buf = String::new();
        if s.is_empty() {
            continue;
        }
        for ch in s.chars() {
            if !buf.is_empty() && buf != "'" && ch.is_uppercase() && !last_upper {
                words.push(buf);
                buf = String::new();
            }
            last_upper = ch.is_uppercase();
            buf.extend(ch.to_lowercase());
        }
        words.push(buf);
    }
    words.join("_")
}

impl FsmTransitionState {
    pub fn get_fsm_state(&self) -> syn::Result<&FsmState> {
        if let FsmTransitionState::State(ref st) = self {
            Ok(st)
        } else {
            Err(syn::Error::new(Span::call_site(), "Missing the required FSM state!"))
        }
    }
}

pub fn ty_append(ty: &syn::Type, suffix: &str) -> syn::Type {
    let s = tokens_to_string(&ty);
    let n = format!("{}{}", s, suffix);
    syn::Type::Path(syn::TypePath {
        qself: None,
        path: syn::Ident::new(&n, ty.span()).into()
    })
}

pub fn assert_no_generics(ty: &syn::Type) -> syn::Result<()> {
    match ty {
        syn::Type::Path(ref tp) => {
            for seg in &tp.path.segments {
                match seg.arguments {
                    syn::PathArguments::None => {},
                    _ => {
                        return Err(syn::Error::new(ty.span(), "Generics aren't supported for state or event types!"));
                    }
                }
            }
        }
        _ => { return Err(syn::Error::new(ty.span(), "Unsupported type.")); }
    }

    Ok(())
}

pub fn get_ty_ident(ty: &syn::Type) -> syn::Result<&syn::Ident> {
    match ty {
        syn::Type::Path(syn::TypePath { path, .. }) if path.segments.len() == 1 => {
            let seg = path.segments.first().unwrap();
            return Ok(&seg.ident);
        },
        _ => Err(syn::Error::new(ty.span(), "Expected a single identifier type."))        
    }
}