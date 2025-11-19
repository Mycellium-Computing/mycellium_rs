use crate::common::{Functionalities, Functionality};
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
        get_functionality_trait_tokens(functionality, tokens);
    }
}

fn get_provider_trait_tokens(
    struct_name: &Ident,
    functionalities: &Functionalities,
) -> proc_macro2::TokenStream {
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
        mycellium_computing::core::messages::ProvidedFunctionality {
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
        .map(|functionality| get_functionality_message_tokens(functionality))
        .collect();

    let provider_name = provider_name.to_string();

    tokens.extend(quote! {
        mycellium_computing::core::messages::ProviderMessage {
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
    let topic_base_name = format_ident!(
        "{}_{}",
        provider_name.to_string(),
        functionality.name.to_string()
    )
    .to_string();
    let topic_req_name = format!("{}_Req", topic_base_name);
    let topic_res_name = format!("{}_Res", topic_base_name);

    println!("Generating topics for service {}", topic_base_name);

    let name_ident = &functionality.name;
    let input_type = if functionality.input_type.is_none() {
        quote::quote! {
            mycellium_computing::core::messages::ProviderRequest::<mycellium_computing::core::messages::EmptyMessage>
        }
    } else {
        let provider_input_type = functionality.input_type.as_ref().unwrap();
        quote::quote! {
            mycellium_computing::core::messages::ProviderRequest::<#provider_input_type>
        }
    };
    let output_type = &functionality.output_type;

    let topic_req_input_type_name = input_type.to_token_stream().to_string();
    let topic_res_output_type_name = output_type.to_token_stream().to_string();

    let topic_tokens = quote! {
        let request_topic = participant.create_topic::<#input_type>(
            #topic_req_name,
            #topic_req_input_type_name,
            dust_dds::infrastructure::qos::QosKind::Default,
            dust_dds::listener::NO_LISTENER,
            dust_dds::infrastructure::status::NO_STATUS,
        )
            .await
            .unwrap();


        let response_topic = participant.create_topic::<#output_type>(
            #topic_res_name,
            #topic_res_output_type_name,
            dust_dds::infrastructure::qos::QosKind::Default,
            dust_dds::listener::NO_LISTENER,
            dust_dds::infrastructure::status::NO_STATUS,
        )
            .await
            .unwrap();

    };

    let reader_tokens = quote! {
        let reader = subscriber.create_datareader::<#input_type>(
            &request_topic,
            dust_dds::infrastructure::qos::QosKind::Default,
            dust_dds::listener::NO_LISTENER,
            dust_dds::infrastructure::status::NO_STATUS
        )
            .await
            .unwrap();
    };

    let writer_tokens = quote! {
        let writer = publisher.create_datawriter::<#output_type>(
            &response_topic,
            dust_dds::infrastructure::qos::QosKind::Default,
            dust_dds::listener::NO_LISTENER,
            dust_dds::infrastructure::status::NO_STATUS
        )
            .await
            .unwrap();
    };

    let method_call = if functionality.input_type.is_none() {
        quote! {
            Self::#name_ident().await
        }
    } else {
        quote! {
            Self::#name_ident(request.payload).await
        }
    };

    let execution_tokens = quote! {
        let mut interval = tokio::time::interval(tick_duration);
        loop {
            let samples = reader.take(
                1,
                dust_dds::infrastructure::sample_info::ANY_SAMPLE_STATE,
                dust_dds::infrastructure::sample_info::ANY_VIEW_STATE,
                dust_dds::infrastructure::sample_info::ANY_INSTANCE_STATE
            ).await;

            if let Ok(requests) = samples {
                let request = requests[0].data().unwrap();
                let response = #method_call;

                writer.write(&response, None).await.unwrap();
            }

            interval.tick().await;
        }
    };

    let name_str = &functionality.name.to_string();

    quote! {
        #name_str => {
            #topic_tokens

            #reader_tokens

            #writer_tokens

            #execution_tokens
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
        .map(|functionality| get_functionality_channel_tokens(&provider_name, functionality));

    tokens.extend(quote! {
        match functionality_name.as_str() {
            #(#functionalities_channel_branches,)*
            _ => panic!("Unknown functionality"),
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

    let runtime = &functionalities.runtime;

    quote::quote! {
        impl mycellium_computing::core::application::provider::ProviderTrait for #provider_name {
            fn get_functionalities() -> mycellium_computing::core::messages::ProviderMessage {
                #message_tokens
            }

            fn run_executor(
                tick_duration: std::time::Duration,
                functionality_name: String,
                participant: &dust_dds::dds_async::domain_participant::DomainParticipantAsync<#runtime>,
                publisher: &dust_dds::dds_async::publisher::PublisherAsync<#runtime>,
                subscriber: &dust_dds::dds_async::subscriber::SubscriberAsync<#runtime>
            ) -> impl Future<Output = ()> + Send {
                async move { #channel_tokens }
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
