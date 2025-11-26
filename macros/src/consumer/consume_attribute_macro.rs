use crate::{
    MACRO_MSG_PREFIX, MACRO_MSG_SUFFIX,
    common::{Functionalities, Functionality, FunctionalityKind},
    naming::{get_empty_message_type_name, get_request_response_topic_type_names, get_topic_names},
};
use proc_macro::TokenStream;
use quote::{ToTokens, format_ident, quote};
use syn::{Ident, ItemStruct, Type};

fn get_functionalities_readers_attributes(
    functionalities: &Functionalities,
) -> Vec<proc_macro2::TokenStream> {
    let runtime = &functionalities.runtime;
    functionalities
        .functionalities
        .iter()
        .filter_map(|functionality| {
            let name = &functionality.name;
            let output_type = &functionality.output_type;

            match functionality.kind {
                FunctionalityKind::Continuous => None, // Continuous functionalities don't have a reader in the struct
                FunctionalityKind::RequestResponse | FunctionalityKind::Response => {
                    let reader_ident = format_ident!("{}_reader", name.to_string().to_lowercase());
                    Some(quote! {
                        #reader_ident: dust_dds::dds_async::data_reader::DataReaderAsync<#runtime, mycellium_computing::core::messages::ProviderExchange<#output_type>>
                    })
                }
            }
        })
        .collect()
}

fn get_functionalities_writers_attributes(
    functionalities: &Functionalities,
) -> Vec<proc_macro2::TokenStream> {
    let runtime = &functionalities.runtime;
    functionalities
        .functionalities
        .iter()
        .filter_map(|functionality| {
            let name = &functionality.name;

            match functionality.kind {
                FunctionalityKind::Continuous => None, // Continuous functionalities don't have a writer in the struct
                FunctionalityKind::RequestResponse => {
                    let writer_ident = format_ident!("{}_writer", name.to_string().to_lowercase());
                    let input_type = functionality.input_type.as_ref().unwrap();
                    Some(quote! {
                        #writer_ident: dust_dds::dds_async::data_writer::DataWriterAsync<#runtime, mycellium_computing::core::messages::ProviderExchange<#input_type>>
                    })
                }
                FunctionalityKind::Response => {
                    let writer_ident = format_ident!("{}_writer", name.to_string().to_lowercase());
                    Some(quote! {
                        #writer_ident: dust_dds::dds_async::data_writer::DataWriterAsync<#runtime, mycellium_computing::core::messages::ProviderExchange<mycellium_computing::core::messages::EmptyMessage>>
                    })
                }
            }
        })
        .collect()
}

fn generate_continuous_topic(name: &Ident, output_type: &Type) -> proc_macro2::TokenStream {
    let topic_name_str = name.to_string().to_lowercase();
    let topic_var_ident = format_ident!("{}_topic", name.to_string().to_lowercase());
    let type_name = quote!(#output_type).to_string();

    quote! {
        let #topic_var_ident = participant.create_topic::<#output_type>(
            #topic_name_str,
            #type_name,
            dust_dds::infrastructure::qos::QosKind::Default,
            dust_dds::listener::NO_LISTENER,
            dust_dds::infrastructure::status::NO_STATUS,
        )
        .await
        .unwrap();
    }
}

fn generate_request_response_topics(
    name: &Ident,
    output_type: &Type,
    request_payload_type: proc_macro2::TokenStream,
    input_type_for_naming: Option<&Type>,
) -> proc_macro2::TokenStream {
    let (topic_req_name, topic_res_name) = get_topic_names(&name.to_string());

    let input_name = if input_type_for_naming.is_some() {
        get_empty_message_type_name()
    } else {
        output_type.to_token_stream().to_string()
    };

    let (topic_req_type_name, topic_res_type_name) = get_request_response_topic_type_names(
        input_name,
        output_type.to_token_stream().to_string(),
    );

    let req_topic_var_ident = format_ident!("{}_req_topic", name.to_string().to_lowercase());
    let res_topic_var_ident = format_ident!("{}_res_topic", name.to_string().to_lowercase());

    quote! {
        let #req_topic_var_ident = participant.create_topic::<mycellium_computing::core::messages::ProviderExchange<#request_payload_type>>(
            #topic_req_name,
            #topic_req_type_name,
            dust_dds::infrastructure::qos::QosKind::Default,
            dust_dds::listener::NO_LISTENER,
            dust_dds::infrastructure::status::NO_STATUS,
        )
        .await
        .unwrap();

        let #res_topic_var_ident = participant.create_topic::<mycellium_computing::core::messages::ProviderExchange<#output_type>>(
            #topic_res_name,
            #topic_res_type_name,
            dust_dds::infrastructure::qos::QosKind::Default,
            dust_dds::listener::NO_LISTENER,
            dust_dds::infrastructure::status::NO_STATUS,
        )
        .await
        .unwrap();
    }
}

fn get_functionalities_topics_instantiations(
    functionalities: &Functionalities,
) -> Vec<proc_macro2::TokenStream> {
    functionalities
        .functionalities
        .iter()
        .map(|functionality: &Functionality| {
            let name = &functionality.name;
            let output_type = &functionality.output_type;

            println!(
                "{}Generating consumer topic for functionality: {}{}",
                MACRO_MSG_PREFIX,
                name.to_string(),
                MACRO_MSG_SUFFIX
            );

            match functionality.kind {
                FunctionalityKind::Continuous => generate_continuous_topic(name, output_type),
                FunctionalityKind::RequestResponse => {
                    let input_type = functionality.input_type.as_ref().unwrap();
                    generate_request_response_topics(
                        name,
                        output_type,
                        quote!(#input_type),
                        functionality.input_type.as_ref(),
                    )
                }
                FunctionalityKind::Response => generate_request_response_topics(
                    name,
                    output_type,
                    quote!(mycellium_computing::core::messages::EmptyMessage),
                    functionality.input_type.as_ref(),
                ),
            }
        })
        .collect()
}

fn generate_continuous_listener(
    struct_name: &Ident,
    runtime: &Ident,
    output_type: &Type,
    func_name: &Ident,
    index: usize,
) -> proc_macro2::TokenStream {
    let output_type_name = quote! { #output_type }.to_string();
    let listener_name = format_ident!("{}Listener{}", output_type_name, index);

    quote! {
        struct #listener_name;
        impl dust_dds::subscription::data_reader_listener::DataReaderListener<#runtime, #output_type> for #listener_name {
            async fn on_data_available(
                &mut self,
                reader: dust_dds::dds_async::data_reader::DataReaderAsync<#runtime, #output_type>,
            ) {
                let samples = reader
                    .take(
                        100, // TODO: Allow the user to specify the number of samples to take
                        dust_dds::infrastructure::sample_info::ANY_SAMPLE_STATE,
                        dust_dds::infrastructure::sample_info::ANY_VIEW_STATE,
                        dust_dds::infrastructure::sample_info::ANY_INSTANCE_STATE,
                    )
                    .await;

                if let Ok(data) = samples {
                    for sample in &data {
                        if let Ok(data) = sample.data() {
                            #struct_name::#func_name(data).await;
                        }
                    }
                }
            }
        }
    }
}

fn get_functionalities_listeners(
    struct_name: &Ident,
    functionalities: &Functionalities,
) -> Vec<proc_macro2::TokenStream> {
    let runtime = &functionalities.runtime;

    functionalities
        .functionalities
        .iter()
        .enumerate()
        .filter(|(_, f)| f.kind == FunctionalityKind::Continuous)
        .map(|(i, functionality)| {
            generate_continuous_listener(
                struct_name,
                runtime,
                &functionality.output_type,
                &functionality.name,
                i,
            )
        })
        .collect()
}

fn generate_request_response_trait_method(
    name: &Ident,
    input_type: &Type,
    output_type: &Type,
) -> proc_macro2::TokenStream {
    quote! {
        async fn #name(
            &self,
            data: #input_type,
            timeout: dust_dds::infrastructure::time::Duration,
        ) -> Option<#output_type>;
    }
}

fn generate_response_trait_method(name: &Ident, output_type: &Type) -> proc_macro2::TokenStream {
    quote! {
        async fn #name(
            &self,
            timeout: dust_dds::infrastructure::time::Duration,
        ) -> Option<#output_type>;
    }
}

fn generate_continuous_trait(
    struct_name: &Ident,
    continuous_funcs: &[&Functionality],
) -> Option<proc_macro2::TokenStream> {
    if continuous_funcs.is_empty() {
        return None;
    }

    let trait_name = format_ident!("{}ContinuosTrait", struct_name);
    let methods = continuous_funcs.iter().map(|f| {
        let name = &f.name;
        let output_type = &f.output_type;
        quote! {
            async fn #name(data: #output_type);
        }
    });

    Some(quote! {
        trait #trait_name {
            #(#methods)*
        }
    })
}

fn generate_response_trait(
    struct_name: &Ident,
    response_funcs: &[&Functionality],
) -> Option<proc_macro2::TokenStream> {
    if response_funcs.is_empty() {
        return None;
    }

    let trait_name = format_ident!("{}ResponseTrait", struct_name);
    let methods = response_funcs.iter().map(|f| {
        let name = &f.name;
        let output_type = &f.output_type;

        match f.kind {
            FunctionalityKind::RequestResponse => {
                let input_type = f.input_type.as_ref().unwrap();
                generate_request_response_trait_method(name, input_type, output_type)
            }
            FunctionalityKind::Response => generate_response_trait_method(name, output_type),
            _ => unreachable!(),
        }
    });

    Some(quote! {
        trait #trait_name {
            #(#methods)*
        }
    })
}

fn get_functionalities_trait_definitions(
    struct_name: &Ident,
    functionalities: &Functionalities,
) -> Vec<proc_macro2::TokenStream> {
    let continuous_funcs: Vec<_> = functionalities
        .functionalities
        .iter()
        .filter(|f| f.kind == FunctionalityKind::Continuous)
        .collect();

    let response_funcs: Vec<_> = functionalities
        .functionalities
        .iter()
        .filter(|f| {
            f.kind == FunctionalityKind::RequestResponse || f.kind == FunctionalityKind::Response
        })
        .collect();

    [
        generate_continuous_trait(struct_name, &continuous_funcs),
        generate_response_trait(struct_name, &response_funcs),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn generate_response_wait_logic(
    writer_ident: &Ident,
    reader_ident: &Ident,
    runtime: &Ident,
    output_type: &Type,
) -> proc_macro2::TokenStream {
    quote! {
        let (sender, receiver) = #runtime::oneshot::<#output_type>();

        let listener = mycellium_computing::core::listener::ProviderResponseListener {
            expected_id: request.id,
            response_sender: Some(sender),
        };

        self.#reader_ident
            .set_listener(Some(listener), &[dust_dds::infrastructure::status::StatusKind::DataAvailable])
            .await
            .unwrap();

        self.#writer_ident
            .write(&request, None)
            .await
            .unwrap();

        let data_future = async { receiver.await.ok() }.fuse();

        let timer_future = mycellium_computing::futures_timer::Delay::new(core::time::Duration::new(
            timeout.sec() as u64,
            timeout.nanosec(),
        ))
        .fuse();

        mycellium_computing::futures::pin_mut!(data_future);
        mycellium_computing::futures::pin_mut!(timer_future);

        mycellium_computing::futures::select! {
            res = data_future => res,
            _ = timer_future => None,
        }
    }
}

fn generate_request_response_method(
    name: &Ident,
    input_type: &Type,
    output_type: &Type,
    writer_ident: &Ident,
    reader_ident: &Ident,
    runtime: &Ident,
) -> proc_macro2::TokenStream {
    let wait_logic = generate_response_wait_logic(writer_ident, reader_ident, runtime, output_type);

    quote! {
        async fn #name(
            &self,
            data: #input_type,
            timeout: dust_dds::infrastructure::time::Duration,
        ) -> Option<#output_type> {
            use dust_dds::runtime::DdsRuntime;
            use mycellium_computing::futures::FutureExt;

            let request = mycellium_computing::core::messages::ProviderExchange {
                id: mycellium_computing::utils::next_request_id(),
                payload: data,
            };

            #wait_logic
        }
    }
}

fn generate_response_method(
    name: &Ident,
    output_type: &Type,
    writer_ident: &Ident,
    reader_ident: &Ident,
    runtime: &Ident,
) -> proc_macro2::TokenStream {
    let wait_logic = generate_response_wait_logic(writer_ident, reader_ident, runtime, output_type);

    quote! {
        async fn #name(
            &self,
            timeout: dust_dds::infrastructure::time::Duration,
        ) -> Option<#output_type> {
            use dust_dds::runtime::DdsRuntime;
            use mycellium_computing::futures::FutureExt;

            let request = mycellium_computing::core::messages::ProviderExchange {
                id: mycellium_computing::utils::next_request_id(),
                payload: mycellium_computing::core::messages::EmptyMessage,
            };

            #wait_logic
        }
    }
}

fn get_functionalities_trait_implementations(
    struct_name: &Ident,
    functionalities: &Functionalities,
    consumer_struct: &Ident,
) -> Vec<proc_macro2::TokenStream> {
    let response_funcs: Vec<_> = functionalities
        .functionalities
        .iter()
        .filter(|f| {
            f.kind == FunctionalityKind::RequestResponse || f.kind == FunctionalityKind::Response
        })
        .collect();

    if response_funcs.is_empty() {
        return Vec::new();
    }

    let trait_name = format_ident!("{}ResponseTrait", struct_name);
    let runtime = &functionalities.runtime;

    let methods = response_funcs.iter().map(|f| {
        let name = &f.name;
        let output_type = &f.output_type;
        let writer_ident = format_ident!("{}_writer", name.to_string().to_lowercase());
        let reader_ident = format_ident!("{}_reader", name.to_string().to_lowercase());

        match f.kind {
            FunctionalityKind::RequestResponse => {
                let input_type = f.input_type.as_ref().unwrap();
                generate_request_response_method(
                    name,
                    input_type,
                    output_type,
                    &writer_ident,
                    &reader_ident,
                    runtime,
                )
            }
            FunctionalityKind::Response => {
                generate_response_method(name, output_type, &writer_ident, &reader_ident, runtime)
            }
            _ => unreachable!(),
        }
    });

    vec![quote! {
        impl #trait_name for #consumer_struct {
            #(#methods)*
        }
    }]
}

fn get_consumer_struct<'a>(
    struct_name: &Ident,
    functionalities: &Functionalities,
) -> (Ident, proc_macro2::TokenStream) {
    let consumer_struct = format_ident!("{}Consumer", struct_name);

    let data_readers_attributes = get_functionalities_readers_attributes(functionalities);
    let data_writers_attributes = get_functionalities_writers_attributes(functionalities);

    let all_attributes: Vec<_> = data_readers_attributes
        .into_iter()
        .chain(data_writers_attributes)
        .collect();

    (
        consumer_struct.clone(),
        quote::quote! {
            struct #consumer_struct {
                #(#all_attributes),*
            }
        },
    )
}

#[inline(always)]
fn get_init_body_writers(funtionalities: &Functionalities) -> Vec<proc_macro2::TokenStream> {
    funtionalities.functionalities.iter().filter_map(|f| {
        let name = &f.name;
        match f.kind {
            FunctionalityKind::RequestResponse | FunctionalityKind::Response => {
                let writer_ident = format_ident!("{}_writer", name.to_string().to_lowercase());
                let req_topic_var_ident = format_ident!("{}_req_topic", name.to_string().to_lowercase());
                let input_type = if f.kind == FunctionalityKind::RequestResponse {
                    let t = f.input_type.as_ref().unwrap();
                    quote!(#t)
                } else {
                    quote!(mycellium_computing::core::messages::EmptyMessage)
                };

                Some(quote! {
                    let #writer_ident = publisher
                        .create_datawriter::<mycellium_computing::core::messages::ProviderExchange<#input_type>>(
                            &#req_topic_var_ident,
                            dust_dds::infrastructure::qos::QosKind::Default,
                            dust_dds::listener::NO_LISTENER,
                            dust_dds::infrastructure::status::NO_STATUS,
                        )
                        .await
                        .unwrap();
                })
            }
            _ => None
        }
    })
    .collect()
}

#[inline(always)]
fn get_init_body_readers(functionalities: &Functionalities) -> Vec<proc_macro2::TokenStream> {
    functionalities.functionalities.iter().filter_map(|f| {
        let name = &f.name;
        match f.kind {
            FunctionalityKind::RequestResponse | FunctionalityKind::Response => {
                let reader_ident = format_ident!("{}_reader", name.to_string().to_lowercase());
                let res_topic_var_ident = format_ident!("{}_res_topic", name.to_string().to_lowercase());
                let output_type = &f.output_type;

                Some(quote! {
                    let #reader_ident = subscriber
                        .create_datareader::<mycellium_computing::core::messages::ProviderExchange<#output_type>>(
                            &#res_topic_var_ident,
                            dust_dds::infrastructure::qos::QosKind::Default,
                            dust_dds::listener::NO_LISTENER,
                            dust_dds::infrastructure::status::NO_STATUS,
                        )
                        .await
                        .unwrap();
                })
            }
            _ => None
        }
    })
    .collect()
}

#[inline(always)]
fn get_init_body_continuous(functionalities: &Functionalities) -> Vec<proc_macro2::TokenStream> {
    functionalities
        .functionalities
        .iter()
        .enumerate()
        .filter_map(|(i, f)| {
            if f.kind == FunctionalityKind::Continuous {
                let output_type = &f.output_type;
                let output_type_name = quote! { #output_type }.to_string();
                let listener_name = format_ident!("{}Listener{}", output_type_name, i);
                let topic_var_ident = format_ident!("{}_topic", f.name.to_string().to_lowercase());
                Some(quote! {
                    subscriber
                        .create_datareader::<#output_type>(
                            &#topic_var_ident,
                            dust_dds::infrastructure::qos::QosKind::Default,
                            Some(#listener_name),
                            &[dust_dds::infrastructure::status::StatusKind::DataAvailable],
                        )
                        .await
                        .unwrap();
                })
            } else {
                None
            }
        })
        .collect()
}

#[inline(always)]
fn get_struct_init_fields(functionalities: &Functionalities) -> Vec<proc_macro2::TokenStream> {
    functionalities
        .functionalities
        .iter()
        .filter_map(|f| match f.kind {
            FunctionalityKind::RequestResponse | FunctionalityKind::Response => {
                let name = &f.name;
                let writer_ident = format_ident!("{}_writer", name.to_string().to_lowercase());
                let reader_ident = format_ident!("{}_reader", name.to_string().to_lowercase());
                Some(quote! {
                    #writer_ident,
                    #reader_ident
                })
            }
            _ => None,
        })
        .collect()
}

#[inline(always)]
fn get_consumer_struct_impl(
    struct_name: &Ident,
    functionalities: &Functionalities,
    consumer_struct_name: &Ident,
) -> proc_macro2::TokenStream {
    let runtime = &functionalities.runtime;

    let data_topics_instantiations = get_functionalities_topics_instantiations(functionalities);
    let init_body_writers = get_init_body_writers(functionalities);
    let init_body_readers = get_init_body_readers(functionalities);
    let init_body_continuous = get_init_body_continuous(functionalities);
    let struct_init_fields = get_struct_init_fields(functionalities);

    quote! {
        impl #struct_name {
            async fn init(
                participant: &dust_dds::dds_async::domain_participant::DomainParticipantAsync<#runtime>,
                subscriber: &dust_dds::dds_async::subscriber::SubscriberAsync<#runtime>,
                publisher: &dust_dds::dds_async::publisher::PublisherAsync<#runtime>,
            ) -> #consumer_struct_name {
                #(#data_topics_instantiations)*

                #(#init_body_writers)*
                #(#init_body_readers)*
                #(#init_body_continuous)*

                #consumer_struct_name {
                    #(#struct_init_fields),*
                }
            }
        }
    }
}

pub fn apply_consume_attribute_macro(
    functionalities: &Functionalities,
    struct_input: &ItemStruct,
) -> TokenStream {
    let struct_name = &struct_input.ident;

    let (consumer_struct_name, consumer_struct) = get_consumer_struct(struct_name, functionalities);
    let listeners = get_functionalities_listeners(struct_name, functionalities);
    let trait_definitions = get_functionalities_trait_definitions(struct_name, functionalities);
    let trait_implementations = get_functionalities_trait_implementations(
        struct_name,
        functionalities,
        &consumer_struct_name,
    );
    let consumer_struct_impl =
        get_consumer_struct_impl(struct_name, functionalities, &consumer_struct_name);

    let expanded = quote::quote! {
        struct #struct_name;

        #consumer_struct

        #(#listeners)*

        #(#trait_definitions)*

        #(#trait_implementations)*

        #consumer_struct_impl
    };

    TokenStream::from(expanded)
}
