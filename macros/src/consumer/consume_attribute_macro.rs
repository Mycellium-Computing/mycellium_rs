use crate::common::{Functionalities, FunctionalityKind};
use proc_macro::TokenStream;
use quote::{format_ident, quote, ToTokens};
use syn::ItemStruct;

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

fn get_functionalities_topics_instantiations(
    functionalities: &Functionalities,
) -> Vec<proc_macro2::TokenStream> {
    functionalities
        .functionalities
        .iter()
        .map(|functionality| {
            let name = &functionality.name;
            let output_type = &functionality.output_type;

            println!("[CONSUMER MACRO] Generating topic for functionality: {}", name.to_string());

            match functionality.kind {
                FunctionalityKind::Continuous => {
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
                FunctionalityKind::RequestResponse => {
                    let req_topic_name_str = format!("{}_Req", name.to_string().to_lowercase());
                    let res_topic_name_str = format!("{}_Res", name.to_string().to_lowercase());
                    
                    let req_topic_var_ident = format_ident!("{}_req_topic", name.to_string().to_lowercase());
                    let res_topic_var_ident = format_ident!("{}_res_topic", name.to_string().to_lowercase());

                    let input_type = functionality.input_type.as_ref().unwrap();

                    let req_type_name = quote!(mycellium_computing::core::messages::ProviderExchange<#input_type>).to_string();
                    let res_type_name = quote!(mycellium_computing::core::messages::ProviderExchange<#output_type>).to_string();

                    quote! {
                        let #req_topic_var_ident = participant.create_topic::<mycellium_computing::core::messages::ProviderExchange<#input_type>>(
                            #req_topic_name_str,
                            #req_type_name,
                            dust_dds::infrastructure::qos::QosKind::Default,
                            dust_dds::listener::NO_LISTENER,
                            dust_dds::infrastructure::status::NO_STATUS,
                        )
                        .await
                        .unwrap();

                        let #res_topic_var_ident = participant.create_topic::<mycellium_computing::core::messages::ProviderExchange<#output_type>>(
                            #res_topic_name_str,
                            #res_type_name,
                            dust_dds::infrastructure::qos::QosKind::Default,
                            dust_dds::listener::NO_LISTENER,
                            dust_dds::infrastructure::status::NO_STATUS,
                        )
                        .await
                        .unwrap();
                    }
                }
                FunctionalityKind::Response => {
                    let req_topic_name_str = format!("{}_Req", name.to_string().to_lowercase());
                    let res_topic_name_str = format!("{}_Res", name.to_string().to_lowercase());

                    let req_topic_var_ident = format_ident!("{}_req_topic", name.to_string().to_lowercase());
                    let res_topic_var_ident = format_ident!("{}_res_topic", name.to_string().to_lowercase());

                    let req_type_name = quote!(mycellium_computing::core::messages::ProviderExchange<mycellium_computing::core::messages::EmptyMessage>).to_string();
                    let res_type_name = quote!(mycellium_computing::core::messages::ProviderExchange<#output_type>).to_string();

                    quote! {
                        let #req_topic_var_ident = participant.create_topic::<mycellium_computing::core::messages::ProviderExchange<mycellium_computing::core::messages::EmptyMessage>>(
                            #req_topic_name_str,
                            #req_type_name,
                            dust_dds::infrastructure::qos::QosKind::Default,
                            dust_dds::listener::NO_LISTENER,
                            dust_dds::infrastructure::status::NO_STATUS,
                        )
                        .await
                        .unwrap();

                        let #res_topic_var_ident = participant.create_topic::<mycellium_computing::core::messages::ProviderExchange<#output_type>>(
                            #res_topic_name_str,
                            #res_type_name,
                            dust_dds::infrastructure::qos::QosKind::Default,
                            dust_dds::listener::NO_LISTENER,
                            dust_dds::infrastructure::status::NO_STATUS,
                        )
                        .await
                        .unwrap();
                    }
                }
            }
        })
        .collect()
}

fn get_functionalities_listener_definitions(
    functionalities: &Functionalities,
    struct_name: &syn::Ident,
) -> Vec<proc_macro2::TokenStream> {
    let runtime = &functionalities.runtime;
    functionalities
        .functionalities
        .iter()
        .enumerate()
        .filter_map(|(i, functionality)| {
            if functionality.kind == FunctionalityKind::Continuous {
                let output_type = &functionality.output_type;
                let output_type_name = quote! { #output_type }.to_string();
                let listener_name = format_ident!("{}Listener{}", output_type_name, i);
                let output_type = &functionality.output_type;
                let func_name = &functionality.name;

                Some(quote! {
                    struct #listener_name;
                    impl dust_dds::subscription::data_reader_listener::DataReaderListener<#runtime, #output_type> for #listener_name {
                        async fn on_data_available(
                            &mut self,
                            reader: dust_dds::dds_async::data_reader::DataReaderAsync<#runtime, #output_type>,
                        ) {
                            let samples = reader
                                .take(
                                    100,
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
                })
            } else {
                None
            }
        })
        .collect()
}

fn get_functionalities_trait_definitions(
    functionalities: &Functionalities,
    struct_name: &syn::Ident,
) -> Vec<proc_macro2::TokenStream> {
    let mut traits = Vec::new();

    // Continuous Trait
    let continuous_funcs: Vec<_> = functionalities
        .functionalities
        .iter()
        .filter(|f| f.kind == FunctionalityKind::Continuous)
        .collect();

    if !continuous_funcs.is_empty() {
        let trait_name = format_ident!("{}ContinuosTrait", struct_name);
        let methods = continuous_funcs.iter().map(|f| {
            let name = &f.name;
            let output_type = &f.output_type;
            quote! {
                async fn #name(data: #output_type);
            }
        });

        traits.push(quote! {
            trait #trait_name {
                #(#methods)*
            }
        });
    }

    // Response Trait
    let response_funcs: Vec<_> = functionalities
        .functionalities
        .iter()
        .filter(|f| f.kind == FunctionalityKind::RequestResponse || f.kind == FunctionalityKind::Response)
        .collect();

    if !response_funcs.is_empty() {
        let trait_name = format_ident!("{}ResponseTrait", struct_name);
        let methods = response_funcs.iter().map(|f| {
            let name = &f.name;
            let output_type = &f.output_type;
            match f.kind {
                FunctionalityKind::RequestResponse => {
                    let input_type = f.input_type.as_ref().unwrap();
                    quote! {
                        async fn #name(
                            &self,
                            data: #input_type,
                            timeout: dust_dds::infrastructure::time::Duration,
                        ) -> Option<#output_type>;
                    }
                }
                FunctionalityKind::Response => {
                    quote! {
                        async fn #name(
                            &self,
                            timeout: dust_dds::infrastructure::time::Duration,
                        ) -> Option<#output_type>;
                    }
                }
                _ => unreachable!(),
            }
        });

        traits.push(quote! {
            trait #trait_name {
                #(#methods)*
            }
        });
    }

    traits
}

fn get_functionalities_trait_implementations(
    functionalities: &Functionalities,
    struct_name: &syn::Ident,
    consumer_struct: &syn::Ident,
) -> Vec<proc_macro2::TokenStream> {
    let mut impls = Vec::new();

    // Response Trait Implementation
    let response_funcs: Vec<_> = functionalities
        .functionalities
        .iter()
        .filter(|f| f.kind == FunctionalityKind::RequestResponse || f.kind == FunctionalityKind::Response)
        .collect();

    if !response_funcs.is_empty() {
        let trait_name = format_ident!("{}ResponseTrait", struct_name);
        let methods = response_funcs.iter().map(|f| {
            let name = &f.name;
            let output_type = &f.output_type;
            let writer_ident = format_ident!("{}_writer", name.to_string().to_lowercase());
            let reader_ident = format_ident!("{}_reader", name.to_string().to_lowercase());
            let runtime = &functionalities.runtime;

            match f.kind {
                FunctionalityKind::RequestResponse => {
                    let input_type = f.input_type.as_ref().unwrap();
                    quote! {
                        async fn #name(
                            &self,
                            data: #input_type,
                            timeout: dust_dds::infrastructure::time::Duration,
                        ) -> Option<#output_type> {
                            let request = mycellium_computing::core::messages::ProviderExchange {
                                id: 0, // For now
                                payload: data,
                            };

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

                            let timer_future = futures_timer::Delay::new(core::time::Duration::new(
                                timeout.sec() as u64,
                                timeout.nanosec(),
                            ))
                            .fuse();

                            futures::pin_mut!(data_future);
                            futures::pin_mut!(timer_future);

                            futures::select! {
                                res = data_future => res,
                                _ = timer_future => None,
                            }
                        }
                    }
                }
                FunctionalityKind::Response => {
                    quote! {
                        async fn #name(
                            &self,
                            timeout: dust_dds::infrastructure::time::Duration,
                        ) -> Option<#output_type> {
                            let request = mycellium_computing::core::messages::ProviderExchange {
                                id: 0, // For now
                                payload: mycellium_computing::core::messages::EmptyMessage,
                            };

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

                            let timer_future = futures_timer::Delay::new(core::time::Duration::new(
                                timeout.sec() as u64,
                                timeout.nanosec(),
                            ))
                            .fuse();

                            futures::pin_mut!(data_future);
                            futures::pin_mut!(timer_future);

                            futures::select! {
                                res = data_future => res,
                                _ = timer_future => None,
                            }
                        }
                    }
                }
                _ => unreachable!(),
            }
        });

        impls.push(quote! {
            impl #trait_name for #consumer_struct {
                #(#methods)*
            }
        });
    }

    impls
}

pub fn apply_consume_attribute_macro(
    functionalities: &Functionalities,
    struct_input: &ItemStruct,
) -> TokenStream {
    let struct_name = &struct_input.ident;
    let runtime = &functionalities.runtime;

    let consumer_struct = format_ident!("{}Consumer", struct_name);

    let data_readers_attributes = get_functionalities_readers_attributes(functionalities);
    let data_writers_attributes = get_functionalities_writers_attributes(functionalities);

    let data_topics_instantiations = get_functionalities_topics_instantiations(functionalities);
    let listener_definitions = get_functionalities_listener_definitions(functionalities, struct_name);
    let trait_definitions = get_functionalities_trait_definitions(functionalities, struct_name);
    let trait_implementations = get_functionalities_trait_implementations(functionalities, struct_name, &consumer_struct);

    let init_body_writers = functionalities.functionalities.iter().filter_map(|f| {
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
    });

    let init_body_readers = functionalities.functionalities.iter().filter_map(|f| {
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
    });

    let init_body_continuous = functionalities.functionalities.iter().enumerate().filter_map(|(i, f)| {
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
    });

    let struct_init_fields = functionalities.functionalities.iter().filter_map(|f| {
         match f.kind {
            FunctionalityKind::RequestResponse | FunctionalityKind::Response => {
                let name = &f.name;
                let writer_ident = format_ident!("{}_writer", name.to_string().to_lowercase());
                let reader_ident = format_ident!("{}_reader", name.to_string().to_lowercase());
                Some(quote! {
                    #writer_ident,
                    #reader_ident
                })
            }
            _ => None
        }
    });

    TokenStream::from(quote! {
        struct #struct_name;

        struct #consumer_struct {
            #(#data_readers_attributes),*,
            #(#data_writers_attributes),*
        }

        #(#listener_definitions)*

        #(#trait_definitions)*

        #(#trait_implementations)*

        impl #struct_name {
            async fn init(
                participant: &dust_dds::dds_async::domain_participant::DomainParticipantAsync<#runtime>,
                subscriber: &dust_dds::dds_async::subscriber::SubscriberAsync<#runtime>,
                publisher: &dust_dds::dds_async::publisher::PublisherAsync<#runtime>,
            ) -> #consumer_struct {
                #(#data_topics_instantiations)*

                #(#init_body_writers)*
                #(#init_body_readers)*
                #(#init_body_continuous)*

                #consumer_struct {
                    #(#struct_init_fields),*
                }
            }
        }
    })
}
