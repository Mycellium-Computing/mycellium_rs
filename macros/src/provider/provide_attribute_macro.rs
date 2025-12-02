use crate::{
    MACRO_MSG_PREFIX, MACRO_MSG_SUFFIX,
    common::{Functionalities, Functionality, FunctionalityKind},
    naming::{get_empty_message_type_name, get_request_response_topic_type_names, get_topic_names},
};
use proc_macro::TokenStream;
use quote::{ToTokens, format_ident, quote};
use syn::{Ident, ItemStruct};

// Tokenize the intermediate representation
fn get_functionality_trait_tokens(
    functionality: &Functionality,
    tokens: &mut proc_macro2::TokenStream,
) {
    let name = &functionality.name;
    let input_type = &functionality.input_type;
    let output_type = &functionality.output_type;

    let func_tokens = if functionality.input_type.is_none() {
        quote::quote! {
            async fn #name() -> #output_type;
        }
    } else {
        quote::quote! {
            async fn #name(input: #input_type) -> #output_type;
        }
    };

    tokens.extend(func_tokens);
}

fn get_functionalities_trait_tokens(
    functionalities: &Functionalities,
    tokens: &mut proc_macro2::TokenStream,
) {
    for functionality in &functionalities.functionalities {
        if functionality.kind == FunctionalityKind::Continuous {
            continue;
        }
        get_functionality_trait_tokens(functionality, tokens);
    }
}

/// Generates the ContinuousHandle struct that holds writers for all continuous functionalities.
/// This struct is returned from register_provider and used to publish continuous data.
fn get_continuous_handle_struct_tokens(
    struct_name: &Ident,
    functionalities: &Functionalities,
) -> proc_macro2::TokenStream {
    let continuous_funcs: Vec<_> = functionalities
        .functionalities
        .iter()
        .filter(|f| f.kind == FunctionalityKind::Continuous)
        .collect();

    if continuous_funcs.is_empty() {
        return proc_macro2::TokenStream::new();
    }

    let runtime = &functionalities.runtime;
    let handle_name = format_ident!("{}ContinuousHandle", struct_name);

    let fields = continuous_funcs.iter().map(|f| {
        let field_name = format_ident!("{}_writer", f.name.to_string().to_lowercase());
        let output_type = &f.output_type;
        quote! {
            #field_name: dust_dds::dds_async::data_writer::DataWriterAsync<#runtime, #output_type>
        }
    });

    quote! {
        /// Handle containing writers for continuous functionalities.
        /// Use this to publish continuous data throughout the provider's lifetime.
        pub struct #handle_name {
            #(#fields),*
        }
    }
}

/// Generates impl block for the ContinuousHandle with methods for each continuous functionality.
fn get_continuous_handle_impl_tokens(
    struct_name: &Ident,
    functionalities: &Functionalities,
) -> proc_macro2::TokenStream {
    let continuous_funcs: Vec<_> = functionalities
        .functionalities
        .iter()
        .filter(|f| f.kind == FunctionalityKind::Continuous)
        .collect();

    if continuous_funcs.is_empty() {
        return proc_macro2::TokenStream::new();
    }

    let handle_name = format_ident!("{}ContinuousHandle", struct_name);

    let methods = continuous_funcs.iter().map(|f| {
        let method_name = &f.name;
        let field_name = format_ident!("{}_writer", f.name.to_string().to_lowercase());
        let output_type = &f.output_type;

        quote! {
            /// Publishes data for this continuous functionality.
            pub async fn #method_name(&self, data: &#output_type) {
                self.#field_name.write(data, None).await.unwrap();
            }
        }
    });

    quote! {
        impl #handle_name {
            #(#methods)*
        }
    }
}

/// Generates the create_continuous_handle implementation that creates writers
/// and returns a populated ContinuousHandle struct.
fn get_create_continuous_handle_impl_tokens(
    struct_name: &Ident,
    functionalities: &Functionalities,
) -> proc_macro2::TokenStream {
    let continuous_funcs: Vec<_> = functionalities
        .functionalities
        .iter()
        .filter(|f| f.kind == FunctionalityKind::Continuous)
        .collect();

    let runtime = &functionalities.runtime;

    if continuous_funcs.is_empty() {
        // No continuous functionalities - return NoContinuousHandle
        return quote! {
            type ContinuousHandle = mycelium_computing::core::application::provider::NoContinuousHandle;

            async fn create_continuous_handle(
                _participant: &dust_dds::dds_async::domain_participant::DomainParticipantAsync<#runtime>,
                _publisher: &dust_dds::dds_async::publisher::PublisherAsync<#runtime>,
            ) -> Self::ContinuousHandle {
                mycelium_computing::core::application::provider::NoContinuousHandle
            }
        };
    }

    let handle_name = format_ident!("{}ContinuousHandle", struct_name);

    // Generate topic creation and writer creation for each continuous functionality
    let topic_creations = continuous_funcs.iter().map(|f| {
        let topic_var = format_ident!("{}_topic", f.name.to_string().to_lowercase());
        let topic_name = f.name.to_string().to_lowercase();
        let output_type = &f.output_type;
        let type_name = quote!(#output_type).to_string();

        quote! {
            let #topic_var = participant.create_topic::<#output_type>(
                #topic_name,
                #type_name,
                dust_dds::infrastructure::qos::QosKind::Default,
                dust_dds::listener::NO_LISTENER,
                dust_dds::infrastructure::status::NO_STATUS,
            )
            .await
            .unwrap();
        }
    });

    let writer_creations = continuous_funcs.iter().map(|f| {
        let writer_var = format_ident!("{}_writer", f.name.to_string().to_lowercase());
        let topic_var = format_ident!("{}_topic", f.name.to_string().to_lowercase());
        let output_type = &f.output_type;

        quote! {
            let #writer_var = publisher.create_datawriter::<#output_type>(
                &#topic_var,
                dust_dds::infrastructure::qos::QosKind::Default,
                dust_dds::listener::NO_LISTENER,
                dust_dds::infrastructure::status::NO_STATUS,
            )
            .await
            .unwrap();
        }
    });

    let field_inits = continuous_funcs.iter().map(|f| {
        let field_name = format_ident!("{}_writer", f.name.to_string().to_lowercase());
        quote! { #field_name }
    });

    quote! {
        type ContinuousHandle = #handle_name;

        async fn create_continuous_handle(
            participant: &dust_dds::dds_async::domain_participant::DomainParticipantAsync<#runtime>,
            publisher: &dust_dds::dds_async::publisher::PublisherAsync<#runtime>,
        ) -> Self::ContinuousHandle {
            #(#topic_creations)*
            #(#writer_creations)*

            #handle_name {
                #(#field_inits),*
            }
        }
    }
}

fn get_provider_trait_tokens(
    struct_name: &Ident,
    functionalities: &Functionalities,
) -> proc_macro2::TokenStream {
    let provider_trait_name = format_ident!("{}ProviderTrait", struct_name);

    let mut trait_tokens = proc_macro2::TokenStream::new();
    get_functionalities_trait_tokens(functionalities, &mut trait_tokens);

    // Generate ContinuousHandle struct and impl
    let continuous_handle_struct =
        get_continuous_handle_struct_tokens(struct_name, functionalities);
    let continuous_handle_impl = get_continuous_handle_impl_tokens(struct_name, functionalities);

    quote! {
        trait #provider_trait_name {
            #trait_tokens
        }

        #continuous_handle_struct

        #continuous_handle_impl
    }
}

fn get_functionality_message_tokens(functionality: &Functionality) -> proc_macro2::TokenStream {
    let name = &functionality.name.to_string();
    let input_type = &functionality.input_type.to_token_stream().to_string();
    let output_type = &functionality.output_type.to_token_stream().to_string();

    quote! {
        mycelium_computing::core::messages::ProvidedFunctionality {
            name: #name.to_string(),
            input_type: #input_type.to_string(),
            output_type: #output_type.to_string(),
        }
    }
}

fn get_functionalities_message_tokens(
    provider_name: &Ident,
    functionalities: &Functionalities,
    tokens: &mut proc_macro2::TokenStream,
) {
    let functionalities_messages: Vec<proc_macro2::TokenStream> = functionalities
        .functionalities
        .iter()
        .filter(|x| x.kind != FunctionalityKind::Continuous)
        .map(|functionality| get_functionality_message_tokens(functionality))
        .collect();

    let provider_name = provider_name.to_string();

    tokens.extend(quote! {
        mycelium_computing::core::messages::ProviderMessage {
            provider_name: #provider_name.to_string(),
            functionalities: vec![
                #(#functionalities_messages),*
            ],
        }
    });
}

fn get_functionality_channel_tokens(
    provider_name: &Ident,
    functionality: &Functionality,
) -> proc_macro2::TokenStream {
    // Names for the topic and types
    let (topic_req_name, topic_res_name) = get_topic_names(&functionality.name.to_string());

    let input_name = if functionality.input_type.is_some() {
        get_empty_message_type_name()
    } else {
        functionality.output_type.to_token_stream().to_string()
    };

    let (request_topic_type_name, response_topic_type_name) = get_request_response_topic_type_names(
        input_name,
        functionality.output_type.to_token_stream().to_string(),
    );

    println!(
        "{}Generating provider topics for service {}{}",
        MACRO_MSG_PREFIX, &functionality.name, MACRO_MSG_SUFFIX
    );

    let name_ident = &functionality.name;
    let input_type = if functionality.input_type.is_none() {
        quote::quote! {
            mycelium_computing::core::messages::EmptyMessage
        }
    } else {
        let provider_input_type = functionality.input_type.as_ref().unwrap();
        quote::quote! {
            #provider_input_type
        }
    };
    let output_type = &functionality.output_type;

    let topic_tokens = quote! {
        let request_topic = participant.create_topic::<mycelium_computing::core::messages::ProviderExchange<#input_type>>(
            #topic_req_name,
            #request_topic_type_name,
            dust_dds::infrastructure::qos::QosKind::Default,
            dust_dds::listener::NO_LISTENER,
            dust_dds::infrastructure::status::NO_STATUS,
        )
            .await
            .unwrap();


        let response_topic = participant.create_topic::<mycelium_computing::core::messages::ProviderExchange<#output_type>>(
            #topic_res_name,
            #response_topic_type_name,
            dust_dds::infrastructure::qos::QosKind::Default,
            dust_dds::listener::NO_LISTENER,
            dust_dds::infrastructure::status::NO_STATUS,
        )
            .await
            .unwrap();

    };

    let method_call = if functionality.input_type.is_none() {
        quote! {
            #provider_name::#name_ident().await
        }
    } else {
        quote! {
            #provider_name::#name_ident(request.payload).await
        }
    };

    let writer_tokens = quote! {
        let writer = publisher.create_datawriter::<mycelium_computing::core::messages::ProviderExchange<#output_type>>(
            &response_topic,
            dust_dds::infrastructure::qos::QosKind::Default,
            dust_dds::listener::NO_LISTENER,
            dust_dds::infrastructure::status::NO_STATUS
        )
            .await
            .unwrap();
    };

    let listener_tokens = quote! {
        let listener = mycelium_computing::core::listener::RequestListener {
            writer,
            implementation: Box::new(|request: mycelium_computing::core::messages::ProviderExchange<#input_type>| {
                Box::pin(async move {
                    let result = #method_call;
                    mycelium_computing::core::messages::ProviderExchange {
                        id: request.id,
                        payload: result,
                    }
                })
            }),
        };
    };

    let reader_tokens = quote! {
        #listener_tokens


        let reader = subscriber.create_datareader::<mycelium_computing::core::messages::ProviderExchange<#input_type>>(
            &request_topic,
            dust_dds::infrastructure::qos::QosKind::Default,
            Some(listener),
            &[dust_dds::infrastructure::status::StatusKind::DataAvailable]
        )
            .await
            .unwrap();
    };

    let name_str = &functionality.name.to_string();

    quote! {
        #name_str => {
            #topic_tokens

            #writer_tokens

            #reader_tokens

            storage.save(request_topic);
            storage.save(response_topic);
            storage.save(reader);
        }
    }
}

fn get_functionalities_channel_tokens(
    provider_name: &Ident,
    functionalities: &Functionalities,
    tokens: &mut proc_macro2::TokenStream,
) {
    let functionalities_channel_branches = functionalities
        .functionalities
        .iter()
        .filter(|x| x.kind != FunctionalityKind::Continuous)
        .map(|functionality| get_functionality_channel_tokens(&provider_name, functionality));

    tokens.extend(quote! {
        match functionality_name.as_str() {
            #(#functionalities_channel_branches,)*
            _ => panic!("Unknown functionality {:?}", functionality_name),
        }
    })
}

fn get_provider_impl_tokens(
    provider_name: &Ident,
    functionalities: &Functionalities,
) -> proc_macro2::TokenStream {
    let mut message_tokens = proc_macro2::TokenStream::new();
    get_functionalities_message_tokens(provider_name, functionalities, &mut message_tokens);

    let mut channel_tokens = proc_macro2::TokenStream::new();
    get_functionalities_channel_tokens(provider_name, functionalities, &mut channel_tokens);

    // Get the create_continuous_handle implementation
    let continuous_handle_impl =
        get_create_continuous_handle_impl_tokens(provider_name, functionalities);

    let runtime = &functionalities.runtime;

    quote::quote! {
        impl mycelium_computing::core::application::provider::ProviderTrait<#runtime> for #provider_name {
            #continuous_handle_impl

            fn get_functionalities() -> mycelium_computing::core::messages::ProviderMessage {
                #message_tokens
            }

            async fn create_execution_objects(
                functionality_name: String,
                participant: &dust_dds::dds_async::domain_participant::DomainParticipantAsync<#runtime>,
                publisher: &dust_dds::dds_async::publisher::PublisherAsync<#runtime>,
                subscriber: &dust_dds::dds_async::subscriber::SubscriberAsync<#runtime>,
                storage: &mut mycelium_computing::utils::storage::ExecutionObjects,
            ) {
                #channel_tokens
            }
        }
    }
}

pub fn apply_provide_attribute_macro(
    functionalities: &Functionalities,
    struct_input: &ItemStruct,
) -> TokenStream {
    let struct_name = &struct_input.ident;
    let provider_trait = get_provider_trait_tokens(struct_name, functionalities);
    let provider_impl = get_provider_impl_tokens(&struct_name, functionalities);

    let expanded = quote::quote! {
        #struct_input

        #provider_impl

        #provider_trait
    };

    TokenStream::from(expanded)
}
