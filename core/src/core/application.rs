pub mod consumer;
pub mod provider;

use crate::core::application::provider::ProviderTrait;
use crate::core::messages::{ConsumerDiscovery, ProviderMessage};
use core::time::Duration;
use dust_dds::dds_async::data_reader::DataReaderAsync;
use dust_dds::dds_async::data_writer::DataWriterAsync;
use dust_dds::dds_async::domain_participant::DomainParticipantAsync;
use dust_dds::dds_async::domain_participant_factory::DomainParticipantFactoryAsync;
use dust_dds::dds_async::publisher::PublisherAsync;
use dust_dds::dds_async::subscriber::SubscriberAsync;
use dust_dds::infrastructure::qos::QosKind;
use dust_dds::infrastructure::status::{NO_STATUS, StatusKind};
use dust_dds::listener::NO_LISTENER;
use dust_dds::std_runtime::StdRuntime;

pub struct Application {
    name: String,
    pub participant: DomainParticipantAsync<StdRuntime>,
    pub publisher: PublisherAsync<StdRuntime>,
    subscriber: SubscriberAsync<StdRuntime>,
    consumer_request_reader: DataReaderAsync<StdRuntime, ConsumerDiscovery>,
    provider_registration_writer: DataWriterAsync<StdRuntime, ProviderMessage>,
    providers: Vec<ProviderMessage>,
}

impl Application {
    pub async fn run_forever<F>(&self, sleep: impl Fn(Duration) -> F)
    where
        F: Future<Output = ()>,
    {
        println!("{} is waiting forever", self.name);
        loop {
            sleep(Duration::from_secs(10)).await;
        }
    }

    pub async fn register_provider<T: ProviderTrait>(&mut self) {
        let functionalities = T::get_functionalities();

        self.provider_registration_writer
            .write(&functionalities, None)
            .await
            .unwrap();

        for functionality in functionalities.functionalities {
            T::create_execution_objects(
                functionality.name,
                &self.participant,
                &self.publisher,
                &self.subscriber,
            )
            .await;
            // TODO: Should return the writer, reader, topic and listener maybe? I suspect that not returning them and somehow maintaining those references alive will kill the service due to rust reference drop
        }
    }

    pub async fn new(domain_id: u32, name: &str) -> Self {
        let participant_factory = DomainParticipantFactoryAsync::get_instance();

        let participant = participant_factory
            .create_participant(domain_id as i32, QosKind::Default, NO_LISTENER, NO_STATUS)
            .await
            .unwrap();

        let provider_registration_topic = participant
            .create_topic::<ProviderMessage>(
                "ProviderRegistration",
                "ProviderRegistration",
                QosKind::Default,
                NO_LISTENER,
                NO_STATUS,
            )
            .await
            .unwrap();

        let consumer_request_topic = participant
            .create_topic::<ConsumerDiscovery>(
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

        // TODO: Define the specific QoS configuration for the writer
        let provider_registration_writer = publisher
            .create_datawriter::<ProviderMessage>(
                &provider_registration_topic,
                QosKind::Default,
                NO_LISTENER,
                NO_STATUS,
            )
            .await
            .unwrap();

        let providers = Vec::new();

        // TODO: Define the specific QoS configuration for the reader
        let consumer_request_reader = subscriber
            .create_datareader::<ConsumerDiscovery>(
                &consumer_request_topic,
                QosKind::Default,
                NO_LISTENER,
                &[StatusKind::DataAvailable],
            )
            .await
            .unwrap();

        Application {
            name: name.to_string(),
            participant,
            publisher,
            subscriber,
            consumer_request_reader,
            provider_registration_writer,
            providers,
        }
    }
}
