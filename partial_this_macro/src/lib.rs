use proc_macro::TokenStream;
use syn::{ItemStruct, parse_macro_input};

mod codegen;
mod config;
use config::PartialConfig;

#[proc_macro_attribute]
pub fn partial(attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as ItemStruct);
    let cfg = parse_macro_input!(attr as PartialConfig);

    match codegen::generate(&item, &cfg) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}
