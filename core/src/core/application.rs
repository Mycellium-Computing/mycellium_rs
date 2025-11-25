pub mod consumer;
pub mod provider;

use crate::core::application::provider::ProviderTrait;
use crate::core::messages::{ConsumerDiscovery, ProviderMessage};
use crate::utils::storage::ExecutionObjects;
use dust_dds::dds_async::data_reader::DataReaderAsync;
use dust_dds::dds_async::data_writer::DataWriterAsync;
use dust_dds::dds_async::domain_participant::DomainParticipantAsync;
use dust_dds::dds_async::domain_participant_factory::DomainParticipantFactoryAsync;
use dust_dds::dds_async::publisher::PublisherAsync;
use dust_dds::dds_async::subscriber::SubscriberAsync;
use dust_dds::infrastructure::qos::QosKind;
use dust_dds::infrastructure::status::{NO_STATUS, StatusKind};
use dust_dds::listener::NO_LISTENER;
use dust_dds::runtime::DdsRuntime;
use dust_dds::transport::interface::TransportParticipantFactory;

pub struct Application<T: DdsRuntime> {
    name: String,
    pub participant: DomainParticipantAsync<T>,
    pub publisher: PublisherAsync<T>,
    subscriber: SubscriberAsync<T>,
    pub consumer_request_reader: DataReaderAsync<T, ConsumerDiscovery>,
    provider_registration_writer: DataWriterAsync<T, ProviderMessage>,
    pub providers: Vec<ProviderMessage>,
    objects_storage: ExecutionObjects,
}

impl<T: DdsRuntime> Application<T> {
    pub async fn run_forever(&self) {
        println!("{} is waiting forever", self.name);
        core::future::pending::<()>().await;
    }

    pub async fn register_provider<P: ProviderTrait<T>>(&mut self) {
        let functionalities = P::get_functionalities();

        self.provider_registration_writer
            .write(&functionalities, None)
            .await
            .unwrap();

        for functionality in functionalities.functionalities {
            P::create_execution_objects(
                functionality.name,
                &self.participant,
                &self.publisher,
                &self.subscriber,
                &mut self.objects_storage,
            )
            .await;

            // TODO: Should return the writer, reader, topic and listener maybe? I suspect that not returning them and somehow maintaining those references alive will kill the service due to rust reference drop
        }
    }

    pub async fn new(
        domain_id: u32,
        name: &str,
        participant_factory: &'static DomainParticipantFactoryAsync<
            T,
            impl TransportParticipantFactory,
        >,
    ) -> Self {
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

        let objects_storage = ExecutionObjects::new();

        Application {
            name: name.to_string(),
            participant,
            publisher,
            subscriber,
            consumer_request_reader,
            provider_registration_writer,
            providers,
            objects_storage,
        }
    }
}
