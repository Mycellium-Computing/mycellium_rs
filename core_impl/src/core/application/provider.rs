use std::time::Duration;
use dust_dds::dds_async::domain_participant::DomainParticipantAsync;
use dust_dds::dds_async::publisher::PublisherAsync;
use dust_dds::dds_async::subscriber::SubscriberAsync;
use dust_dds::std_runtime::StdRuntime;
use crate::core::application::messages::{ProviderMessage};

pub trait ProviderTrait {

    fn get_functionalities() -> ProviderMessage;

    fn run_executor(
        tick_duration: Duration,
        functionality_name: String,
        participant: &DomainParticipantAsync<StdRuntime>,
        publisher: &PublisherAsync<StdRuntime>,
        subscriber: &SubscriberAsync<StdRuntime>
    ) -> impl Future<Output = ()> + Send;
}