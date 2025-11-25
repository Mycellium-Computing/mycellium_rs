mod common;
mod consumer;
mod provider;

use crate::common::Functionalities;
use crate::consumer::consume_attribute_macro::apply_consume_attribute_macro;
use crate::provider::provide_attribute_macro::apply_provide_attribute_macro;
use proc_macro::TokenStream;
use syn::{ItemStruct, parse_macro_input};

const MACRO_MSG_PREFIX: &str = "\t\x1b[1m\x1b[38;2;33;188;255m[MACRO] ";
const MACRO_MSG_SUFFIX: &str = "\x1b[0m";

#[proc_macro_attribute]
pub fn provides(attr: TokenStream, item: TokenStream) -> TokenStream {
    let functionalities = parse_macro_input!(attr as Functionalities);
    let struct_input = parse_macro_input!(item as ItemStruct);

    apply_provide_attribute_macro(&functionalities, &struct_input)
}

#[proc_macro_attribute]
pub fn consumes(attr: TokenStream, item: TokenStream) -> TokenStream {
    let functionalities = parse_macro_input!(attr as Functionalities);
    let struct_input = parse_macro_input!(item as ItemStruct);

    apply_consume_attribute_macro(&functionalities, &struct_input)
}
