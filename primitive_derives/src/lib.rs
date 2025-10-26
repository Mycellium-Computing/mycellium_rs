mod provider_macro;
mod provider;

use proc_macro::{TokenStream};
use syn::{parse_macro_input, ItemStruct};
use crate::provider::provide_attribute_macro::{apply_provide_attribute_macro, Functionalities};

#[proc_macro_derive(Provider)]
pub fn provider_derive(input: TokenStream) -> TokenStream {
    // provider_macro::apply_provider_macro(input)
    let output = TokenStream::new();

    output
}

#[proc_macro_attribute]
pub fn provides(attr: TokenStream, item: TokenStream) -> TokenStream {
    let values = parse_macro_input!(attr as Functionalities);
    let struct_input = parse_macro_input!(item as ItemStruct);

    apply_provide_attribute_macro(&values, &struct_input)
}