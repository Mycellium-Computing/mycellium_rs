use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::ItemStruct;
use crate::common::Functionalities;

fn get_functionalities_readers_attributes(functionalities: &Functionalities) -> Vec<proc_macro2::TokenStream> {
    let runtime = &functionalities.runtime;
    functionalities.functionalities.iter().map(|functionality| {
        let name = &functionality.name;
        let reader_ident = format_ident!("{}_reader", name.to_string().to_lowercase());
        let output_type = &functionality.output_type;

        quote! {
            #reader_ident: dust_dds::dds_async::data_reader::DataReaderAsync<#runtime, #output_type>
        }
    }).collect()
}

fn get_functionalities_writers_attributes(functionalities: &Functionalities) -> Vec<proc_macro2::TokenStream> {
    let runtime = &functionalities.runtime;
    functionalities.functionalities.iter().map(|functionality| {
        let name = &functionality.name;
        let writer_ident = format_ident!("{}_writer", name.to_string().to_lowercase());
        let input_type = &functionality.input_type;

        quote! {
            #writer_ident: dust_dds::dds_async::data_writer::DataWriterAsync<#runtime, #input_type>
        }
    }).collect()
}

fn get_functionalities_topics_instantiations(functionalities: &Functionalities) -> Vec<proc_macro2::TokenStream> {
    functionalities.functionalities.iter().map(|functionality| {
        let name = &functionality.name;

        let req_topic_name = format_ident!("{}_Req", name.to_string().to_lowercase()).to_string();
        let res_topic_name = format_ident!("{}_Res", name.to_string().to_lowercase()).to_string();

        let output_type = &functionality.output_type;
        let input_type = &functionality.input_type;

        // The consumer writes to the request topic and reads from the response topic

        quote! {
            let req_topic = participant.create_topic::<#input_type>(
                #req_topic_name,
                #input_type,
                dust_dds::infrastructure::qos::QosKind::Default,
                dust_dds::listener::NO_LISTENER,
                dust_dds::infrastructure::status::NO_STATUS,
            )
            .await
            .unwrap();

            let res_topic = participant.create_topic::<#output_type>(
                #res_topic_name,
                #output_type,
                dust_dds::infrastructure::qos::QosKind::Default,
                dust_dds::listener::NO_LISTENER,
                dust_dds::infrastructure::status::NO_STATUS,
            )
            .await
            .unwrap();
        }
    }).collect()
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

    TokenStream::from(quote! {
        struct #struct_name;

        struct #consumer_struct {
            #(#data_readers_attributes),*,
            #(#data_writers_attributes),*
        }

        impl mycellium_computing::core::application::consumer::Consumer for #struct_name {
            async fn init(
                participant: &dust_dds::dds_async::domain_participant::DomainParticipantAsync<#runtime>,
                subscriber: &dust_dds::dds_async::subscriber::SubscriberAsync<#runtime>,
                publisher: &dust_dds::dds_async::publisher::PublisherAsync<#runtime>,
            ) -> #consumer_struct {
                #(#data_topics_instantiations)*

                #consumer_struct {
                }
            }
        }

        impl #struct_name {
        }
    })
}