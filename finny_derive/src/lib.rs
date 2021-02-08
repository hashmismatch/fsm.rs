extern crate proc_macro;
extern crate proc_macro2;

extern crate syn;
extern crate quote;

use codegen::generate_fsm_code;
use parse::FsmFnInput;
use proc_macro::TokenStream;

mod codegen;
mod codegen_meta;
mod meta;
mod parse;
mod parse_blocks;
mod parse_fsm;
mod utils;
mod validation;
mod fsm;


#[proc_macro_attribute]
pub fn finny_fsm(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr2: proc_macro2::TokenStream = attr.into();
    let item2: proc_macro2::TokenStream = item.into();

    let parsed = match FsmFnInput::parse(attr2.clone(), item2.clone()) {
        Ok(p) => p,
        Err(e) => return e.to_compile_error().into()
    };

    match generate_fsm_code(&parsed, attr2.clone(), item2.clone()) {
        Ok(t) => t.into(),
        Err(e) => e.to_compile_error().into()
    }
}
