use proc_macro::TokenStream;
use quote::{format_ident, quote, ToTokens};
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
        content.parse::<Token![,]>()?;
        let input_type: Type = content.parse()?;
        content.parse::<Token![,]>()?;
        let output_type: Type = content.parse()?;

        let name = Ident::new(&name_lit.value(), name_lit.span());

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

// Tokenize the intermediate representation

fn get_functionality_trait_tokens(functionality: &Functionality, tokens: &mut proc_macro2::TokenStream) {
    let name = &functionality.name;
    let input_type = &functionality.input_type;
    let output_type = &functionality.output_type;

    let func_tokens = quote::quote! {
        fn #name(&self, input: #input_type) -> #output_type;
    };

    tokens.extend(func_tokens);
}

fn get_functionalities_trait_tokens(functionalities: &Functionalities, tokens: &mut proc_macro2::TokenStream) {
    for functionality in &functionalities.functionalities {
        get_functionality_trait_tokens(functionality, tokens);
    }
}


fn get_provider_trait_tokens(struct_name: &Ident, functionalities: &Functionalities) -> proc_macro2::TokenStream {
    let provider_trait_name = format_ident!("{}ProviderTrait", struct_name);

    let mut trait_tokens = proc_macro2::TokenStream::new();
    get_functionalities_trait_tokens(functionalities, &mut trait_tokens);

    quote! {
        trait #provider_trait_name {
            #trait_tokens
        }
    }
}

fn get_functionality_message_tokens(functionality: &Functionality) -> proc_macro2::TokenStream {
    let name = &functionality.name.to_string();
    let input_type = &functionality.input_type.to_token_stream().to_string();
    let output_type = &functionality.output_type.to_token_stream().to_string();

    quote! {
        modular_architecture::core::application::messages::ProvidedFunctionality {
            name: #name.to_string(),
            input_type: #input_type.to_string(),
            output_type: #output_type.to_string(),
        }
    }
}

fn get_functionalities_message_tokens(provider_name: &Ident, functionalities: &Functionalities, tokens: &mut proc_macro2::TokenStream) {
    let functionalities_messages: Vec<proc_macro2::TokenStream> = functionalities.functionalities.iter().map(|functionality| {
        get_functionality_message_tokens(functionality)
    }).collect();

    let provider_name = provider_name.to_string();

    tokens.extend(quote! {
        modular_architecture::core::application::messages::ProviderMessage {
            provider_name: #provider_name.to_string(),
            functionalities: vec![
                #(#functionalities_messages),*
            ],
        }
    });
}

fn get_functionality_match_tokens(functionality: &Functionality) -> proc_macro2::TokenStream {
    let name_str = &functionality.name.to_string();
    let name_ident = &functionality.name;
    let input_type = &functionality.input_type;
    quote! {
        #name_str => {
            let input = *input.downcast::<#input_type>().unwrap();
            Box::new(self.#name_ident(input))
        }
    }
}

fn get_functionalities_match_tokens(functionalities: &Functionalities, tokens: &mut proc_macro2::TokenStream) {
    let functionalities_match_branches = functionalities.functionalities.iter().map(|functionality| {
        get_functionality_match_tokens(functionality)
    });

    tokens.extend(quote! {
        match method {
            #(#functionalities_match_branches,)*
            _ => panic!("Unknown method"),
        }
    })
}

fn get_provider_impl_tokens(
    provider_name: &Ident,
    _provider_trait_name: &Ident,
    functionalities: &Functionalities,
) -> proc_macro2::TokenStream {
    let mut message_tokens = proc_macro2::TokenStream::new();
    get_functionalities_message_tokens(provider_name, functionalities, &mut message_tokens);

    let mut execute_tokens = proc_macro2::TokenStream::new();
    get_functionalities_match_tokens(functionalities, &mut execute_tokens);

    quote::quote! {
        impl modular_architecture::core::application::provider::ProviderTrait for #provider_name {
            fn get_functionalities() -> modular_architecture::core::application::messages::ProviderMessage {
                #message_tokens
            }

            fn execute(&self, method: &str, input: Box<dyn std::any::Any>) -> Box<dyn std::any::Any> {
                #execute_tokens
            }
        }
    }
}

pub fn apply_provide_attribute_macro(
    functionalities: &Functionalities,
    struct_input: &syn::ItemStruct,
) -> TokenStream {
    let struct_name = &struct_input.ident;
    let provider_trait_name = format_ident!("{}ProviderTrait", struct_name);
    let provider_trait = get_provider_trait_tokens(struct_name, functionalities);
    let provider_impl = get_provider_impl_tokens(&struct_name, &provider_trait_name, functionalities);

    let expanded = quote::quote! {
        #struct_input

        #provider_impl

        #provider_trait
    };

    TokenStream::from(expanded)
}