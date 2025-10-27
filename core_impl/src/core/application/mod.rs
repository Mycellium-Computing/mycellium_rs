pub mod messages;
pub mod provider;

use dust_dds::domain::domain_participant::DomainParticipant;
use dust_dds::domain::domain_participant_factory::DomainParticipantFactory;
use dust_dds::infrastructure::qos::QosKind;
use dust_dds::infrastructure::status::NO_STATUS;
use dust_dds::listener::NO_LISTENER;
use dust_dds::std_runtime::StdRuntime;
use dust_dds::topic_definition::topic_description::TopicDescription;
use crate::core::application::messages::{ConsumerRequest, ProviderMessage};
use crate::core::application::provider::ProviderTrait;

pub struct Application {
    name: String,
    participant: DomainParticipant<StdRuntime>,
    provider_registration_topic: TopicDescription<StdRuntime>,
    customer_request_topic: TopicDescription<StdRuntime>,

    providers: Vec<Box<dyn ProviderTrait>>,
}

impl Application {
    pub fn register_provider<T>(&mut self)
    where
        T: ProviderTrait + Default + 'static,
    {
        let provider = T::default();
        self.providers.push(Box::new(provider));
    }

    pub fn run(&self) {
        self.setup_providers();
    }

    pub fn new(domain_id: u32, name: &str) -> Self {
        let participant_factory = DomainParticipantFactory::get_instance();

        let participant = participant_factory
            .create_participant(domain_id as i32, QosKind::Default, NO_LISTENER, NO_STATUS)
            .unwrap();

        let provider_registration_topic = participant.create_topic::<ProviderMessage>(
            "ProviderRegistration",
            "ProviderRegistration",
            QosKind::Default,
            NO_LISTENER,
            NO_STATUS,
        ).unwrap();

        let consumer_request_topic = participant.create_topic::<ConsumerRequest>(
            "ConsumerRequest",
            "ConsumerRequest",
            QosKind::Default,
            NO_LISTENER,
            NO_STATUS,
        ).unwrap();

        Application {
            name: name.to_string(),
            participant,
            provider_registration_topic,
            customer_request_topic: consumer_request_topic,
            providers: Vec::new(),
        }
    }

    // SetUp providers and threads

    fn setup_providers(&self) {
        for provider in &self.providers {
            let publisher = self.participant
                .create_publisher(QosKind::Default, NO_LISTENER, NO_STATUS)
                .unwrap();

            let writer = publisher
                .create_datawriter::<ProviderMessage>(&self.provider_registration_topic, QosKind::Default, NO_LISTENER, NO_STATUS)
                .unwrap();

            writer.write(&provider.get_functionalities(), None).unwrap();
            println!("Registered provider: {}", provider.get_functionalities().provider_name);
        }
    }
}