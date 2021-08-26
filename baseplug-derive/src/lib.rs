use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};
use syn::parse::{Parse, ParseStream};
use syn::Result;
use core::ops::Not as _;

mod model;

struct MultiDeriveInput (
    Vec<DeriveInput>,
);

impl Parse for MultiDeriveInput {
    fn parse (input: ParseStream<'_>)
      -> Result<Self>
    {
        let mut ret = vec![];
        while input.is_empty().not() {
            ret.push(input.parse()?);
        }
        Ok(Self(ret))
    }
}

#[proc_macro]
pub fn model(input: TokenStream) -> TokenStream {
    let MultiDeriveInput(inputs) = parse_macro_input!(input);
    inputs
        .into_iter()
        .map(model::derive)
        .map(TokenStream::from)
        .collect()
}

#[proc_macro_derive(Parameters, attributes(model, parameter, unsmoothed))]
pub fn derive_parameters(_input: TokenStream) -> TokenStream {
    TokenStream::new()
}
