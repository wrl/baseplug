use proc_macro::TokenStream;
use syn::{parse_macro_input};

mod model;

#[proc_macro]
pub fn model(input: TokenStream) -> TokenStream {
    model::derive(parse_macro_input!(input))
        .into()
}

#[proc_macro_derive(Parameters, attributes(model, parameter, unsmoothed))]
pub fn derive_parameters(_input: TokenStream) -> TokenStream {
    TokenStream::new()
}
