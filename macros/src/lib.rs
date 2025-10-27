mod provider;

use proc_macro::{TokenStream};
use syn::{parse_macro_input, ItemStruct};
use crate::provider::provide_attribute_macro::{apply_provide_attribute_macro, Functionalities};

#[proc_macro_attribute]
pub fn provides(attr: TokenStream, item: TokenStream) -> TokenStream {
    let functionalities = parse_macro_input!(attr as Functionalities);
    let struct_input = parse_macro_input!(item as ItemStruct);

    apply_provide_attribute_macro(&functionalities, &struct_input)
}