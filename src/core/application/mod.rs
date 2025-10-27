pub mod messages;
pub mod provider;

use std::any::Any;
use dust_dds::domain::domain_participant::DomainParticipant;
use dust_dds::domain::domain_participant_factory::DomainParticipantFactory;
use dust_dds::infrastructure::qos::QosKind;
use dust_dds::infrastructure::status::NO_STATUS;
use dust_dds::listener::NO_LISTENER;
use dust_dds::std_runtime::StdRuntime;
use dust_dds::topic_definition::topic_description::TopicDescription;
use crate::core::application::messages::{ConsumerRequest, ProviderMessage};
use crate::core::application::provider::ProviderTrait;
use crate::VisualObjectDetection;


pub struct Application {
    domain_id: u32,
    name: String,
    participant: DomainParticipant<StdRuntime>,
    provider_registration_topic: TopicDescription<StdRuntime>,
    customer_request_topic: TopicDescription<StdRuntime>,

    providers: Vec<Box<dyn Any>>,
}

impl Application {
    pub fn register_provider<T>(&mut self)
    where
        T: ProviderTrait + Default + 'static,
    {
        let provider = T::default();
        self.providers.push(Box::new(provider));
    }

    pub fn run(&mut self) {
        println!("It works!");
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
            domain_id,
            name: name.to_string(),
            participant,
            provider_registration_topic,
            customer_request_topic: consumer_request_topic,
            providers: Vec::new(),
        }
    }
}