use crate::core::messages::ProviderMessage;
use crate::utils::storage::ExecutionObjects;
use dust_dds::dds_async::domain_participant::DomainParticipantAsync;
use dust_dds::dds_async::publisher::PublisherAsync;
use dust_dds::dds_async::subscriber::SubscriberAsync;
use dust_dds::runtime::DdsRuntime;

// TODO: Generalize over any disruptiveness
pub trait ProviderTrait<R: DdsRuntime> {
    fn get_functionalities() -> ProviderMessage;

    fn create_execution_objects(
        functionality_name: String,
        participant: &DomainParticipantAsync<R>,
        publisher: &PublisherAsync<R>,
        subscriber: &SubscriberAsync<R>,
        storage: &mut ExecutionObjects,
    ) -> impl Future<Output = ()>;
}
