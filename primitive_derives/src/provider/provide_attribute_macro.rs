use proc_macro::TokenStream;
use quote::{format_ident, ToTokens};
use syn::parse::Parse;
use syn::punctuated::Punctuated;
use syn::token::Comma;
use syn::{Token, Type, Ident};


// Intermediate representation

pub struct Functionality {
    name: Ident,
    input_type: Type,
    output_type: Type,
}

impl Parse for Functionality {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let content;
        syn::parenthesized!(content in input);

        let name_lit: syn::LitStr = content.parse()?;
        content.parse::<syn::Token![,]>()?;
        let input_type: Type = content.parse()?;
        content.parse::<syn::Token![,]>()?;
        let output_type: Type = content.parse()?;

        let name = syn::Ident::new(&name_lit.value(), name_lit.span());

        Ok(Functionality {
            name,
            input_type,
            output_type,
        })
    }
}

pub struct Functionalities {
    functionalities: Vec<Functionality>
}

impl Parse for Functionalities {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let content;
        syn::bracketed!(content in input);

        let functionalities_parsed: Punctuated<Functionality, Comma> =
            content.parse_terminated(Functionality::parse, Token![,])?;

        let functionalities = functionalities_parsed.into_iter().collect();

        Ok(Functionalities { functionalities })
    }
}

// Tokenize the intermidiate representation

impl ToTokens for Functionality {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let name = &self.name;
        let input_type = &self.input_type;
        let output_type = &self.output_type;

        let func_tokens = quote::quote! {
            fn #name(&self, input: #input_type) -> #output_type;
        };

        tokens.extend(func_tokens);
    }
}

impl ToTokens for Functionalities {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        for functionality in &self.functionalities {
            functionality.to_tokens(tokens);
        }
    }
}

pub fn apply_provide_attribute_macro(
    functionalities: &Functionalities,
    struct_input: &syn::ItemStruct,
) -> TokenStream {
    let struct_name = &struct_input.ident;
    let provider_trait_name = format_ident!("{}ProviderTrait", struct_name);

    let mut trait_tokens = proc_macro2::TokenStream::new();
    functionalities.to_tokens(&mut trait_tokens);

    let expanded = quote::quote! {
        #struct_input

        trait #provider_trait_name {
            #trait_tokens
        }
    };

    TokenStream::from(expanded)
}