use proc_macro::{TokenStream};
use quote::format_ident;
use syn::{parse_macro_input, DeriveInput};

struct Provider {
    name: String,
}


fn tokenize_provider(provider: &Provider) -> TokenStream {
    let name = &provider.name;
    let struct_name = format_ident!("{}Provider", name);

    let expanded = quote::quote! {
        #[derive(DdsType)]
        struct #struct_name {
            #[dust_dds(key)]
            name: String,
        }
    };
    TokenStream::from(expanded)
}


pub fn apply_provider_macro(input: TokenStream) -> TokenStream {
    let input: DeriveInput = parse_macro_input!(input);

    let provider = Provider {
        name: input.ident.to_string(),
    };


    tokenize_provider(&provider)
}