use syn::parse::Parse;
use syn::punctuated::Punctuated;
use syn::token::Comma;
use syn::{Ident, Token, Type};

// Intermediate representation

pub enum FunctionalityKind {
    Continuous,
    RequestResponse,
}

// Example of the tokens' representation:
// Continuous("sensor_data", OutputType)
// RequestResponse("service_name", RequestType, ResponseType)
// RequestResponse("service_name", None, ResponseType)
pub struct Functionality {
    pub name: Ident,
    pub input_type: Type,
    pub output_type: Type,
    pub kind: FunctionalityKind,
}

impl Parse for Functionality {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        // Parse the enum variant first
        let kind_ident: Ident = input.parse()?;

        // Determine which kind based on the identifier
        let kind = match kind_ident.to_string().as_str() {
            "Continuous" => FunctionalityKind::Continuous,
            "RequestResponse" => FunctionalityKind::RequestResponse,
            _ => {
                return Err(syn::Error::new(
                    kind_ident.span(),
                    format!(
                        "expected `Continuous` or `RequestResponse`, found `{}`",
                        kind_ident
                    ),
                ));
            }
        };

        // Parse the content inside the parentheses
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
            kind,
        })
    }
}

// Before: #[provides([...])]
// Now: #[provides(runtime, [...])]
pub struct Functionalities {
    pub runtime: Ident,
    pub functionalities: Vec<Functionality>,
}

impl Parse for Functionalities {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        // Parse the runtime identifier first
        let runtime: Ident = if input.peek(Token![,]) || input.peek(syn::token::Bracket) {
            return Err(syn::Error::new(
                input.span(),
                "Proportionate a runtime identifier before the functionalities list",
            ));
        } else {
            input.parse()?
        };

        // Expect a comma after the runtime identifier
        input.parse::<Token![,]>()?;

        // Parse the bracketed list of functionalities
        let content;
        syn::bracketed!(content in input);

        let functionalities_parsed: Punctuated<Functionality, Comma> =
            content.parse_terminated(Functionality::parse, Token![,])?;

        let functionalities = functionalities_parsed.into_iter().collect();

        Ok(Functionalities {
            runtime,
            functionalities,
        })
    }
}
