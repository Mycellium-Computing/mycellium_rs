pub mod provider;
pub mod consumer;

use std::sync::Arc;
use std::time::Duration;
use dust_dds::dds_async::data_reader::DataReaderAsync;
use dust_dds::dds_async::data_writer::DataWriterAsync;
use dust_dds::dds_async::domain_participant::DomainParticipantAsync;
use dust_dds::dds_async::domain_participant_factory::DomainParticipantFactoryAsync;
use dust_dds::dds_async::publisher::PublisherAsync;
use dust_dds::dds_async::subscriber::SubscriberAsync;
use dust_dds::infrastructure::qos::QosKind;
use dust_dds::infrastructure::sample_info::{ANY_INSTANCE_STATE, ANY_SAMPLE_STATE, ANY_VIEW_STATE};
use dust_dds::infrastructure::status::NO_STATUS;
use dust_dds::listener::NO_LISTENER;
use dust_dds::std_runtime::StdRuntime;
use crate::core::messages::{ConsumerDiscovery, ProviderMessage};
use crate::core::application::provider::ProviderTrait;

pub struct Application {
    name: String,
    tick_duration: Duration,
    participant: Arc<DomainParticipantAsync<StdRuntime>>,
    publisher: Arc<PublisherAsync<StdRuntime>>,
    subscriber: Arc<SubscriberAsync<StdRuntime>>,
    consumer_request_reader: Arc<DataReaderAsync<StdRuntime, ConsumerDiscovery>>,
    provider_registration_writer: Arc<DataWriterAsync<StdRuntime, ProviderMessage>>,
}

impl Application {
    pub async fn run_forever(&self) {
        println!("{} is waiting forever", self.name);
        loop {
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    }


    pub async fn register_provider<T: ProviderTrait>(&mut self)
    {
        // Registerer task
        tokio::spawn({
            let writer = Arc::clone(&self.provider_registration_writer);
            let reader = Arc::clone(&self.consumer_request_reader);
            let mut interval = tokio::time::interval(self.tick_duration);
            async move {
                let functionalities = T::get_functionalities();
                writer.write(&functionalities, None).await.unwrap();
                println!("Registered provider: {}", functionalities.provider_name);

                loop {
                    let samples = reader.take(
                        1, ANY_SAMPLE_STATE, ANY_VIEW_STATE, ANY_INSTANCE_STATE
                    ).await;

                    if let Ok(_) = samples {
                        writer.write(&functionalities, None).await.unwrap();
                    }

                    interval.tick().await;
                }
            }
        });

        for functionality in T::get_functionalities().functionalities {
            tokio::spawn({
                let participant = Arc::clone(&self.participant);
                let publisher = Arc::clone(&self.publisher);
                let subscriber = Arc::clone(&self.subscriber);
                let tick_duration = self.tick_duration.clone();

                async move {
                    T::run_executor(
                        tick_duration,
                        functionality.name,
                        &participant,
                        &publisher,
                        &subscriber
                    ).await
                }
            });
        }

        // Processing direct messages
        tokio::spawn({
            let mut interval = tokio::time::interval(self.tick_duration);
            async move {
                loop {
                    // Check for the consumer discovery messages and if
                    T::get_functionalities();
                    interval.tick().await;
                }
            }
        });
    }

    pub async fn new(domain_id: u32, name: &str, tick_duration: Duration) -> Self {
        let participant_factory = DomainParticipantFactoryAsync::get_instance();

        let participant = participant_factory
            .create_participant(domain_id as i32, QosKind::Default, NO_LISTENER, NO_STATUS)
            .await
            .unwrap();

        let provider_registration_topic = participant.create_topic::<ProviderMessage>(
            "ProviderRegistration",
            "ProviderRegistration",
            QosKind::Default,
            NO_LISTENER,
            NO_STATUS,
        )
            .await
            .unwrap();

        let consumer_request_topic = participant.create_topic::<ConsumerDiscovery>(
            "ConsumerDiscovery",
            "ConsumerDiscovery",
            QosKind::Default,
            NO_LISTENER,
            NO_STATUS,
        )
            .await
            .unwrap();

        let publisher = participant
            .create_publisher(QosKind::Default, NO_LISTENER, NO_STATUS)
            .await
            .unwrap();

        let subscriber = participant
            .create_subscriber(QosKind::Default, NO_LISTENER, NO_STATUS)
            .await
            .unwrap();

        let consumer_request_reader = subscriber
            .create_datareader::<ConsumerDiscovery>(&consumer_request_topic, QosKind::Default, NO_LISTENER, NO_STATUS)
            .await
            .unwrap();

        let provider_registration_writer = publisher
            .create_datawriter::<ProviderMessage>(&provider_registration_topic, QosKind::Default, NO_LISTENER, NO_STATUS)
            .await
            .unwrap();

        Application {
            name: name.to_string(),
            tick_duration,
            participant: Arc::new(participant),
            publisher: Arc::new(publisher),
            subscriber: Arc::new(subscriber),
            consumer_request_reader: Arc::new(consumer_request_reader),
            provider_registration_writer: Arc::new(provider_registration_writer),
        }
    }
}