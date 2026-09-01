use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemStruct, parse_macro_input};

mod config;
use config::PartialConfig;

#[proc_macro_attribute]
pub fn partial(attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as ItemStruct);
    let cfg = parse_macro_input!(attr as PartialConfig);
    // TODO: use `cfg.module_name(&item)` to generate the implementation module.
    let _module = cfg.module_name(&item);
    let out = quote! {
        #item
    };
    TokenStream::from(out)
}
