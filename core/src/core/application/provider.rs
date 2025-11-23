use crate::core::messages::ProviderMessage;
use dust_dds::dds_async::domain_participant::DomainParticipantAsync;
use dust_dds::dds_async::publisher::PublisherAsync;
use dust_dds::dds_async::subscriber::SubscriberAsync;
use dust_dds::std_runtime::StdRuntime;

pub trait ProviderTrait {
    fn get_functionalities() -> ProviderMessage;

    fn create_execution_objects(
        functionality_name: String,
        participant: &DomainParticipantAsync<StdRuntime>,
        publisher: &PublisherAsync<StdRuntime>,
        subscriber: &SubscriberAsync<StdRuntime>,
    ) -> impl Future<Output = ()>;
}
