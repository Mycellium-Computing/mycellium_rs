use dust_dds::infrastructure::type_support::{DdsType, TypeSupport};
use dust_dds::xtypes::deserialize::XTypesDeserialize;
use dust_dds::xtypes::serialize::XTypesSerialize;

#[derive(DdsType, Debug, Clone)]
pub struct ProvidedFunctionality {
    pub name: String,
    pub input_type: String,
    pub output_type: String,
}

#[derive(DdsType, Debug, Clone)]
pub struct ProviderMessage {
    #[dust_dds(key)]
    pub provider_name: String,
    pub functionalities: Vec<ProvidedFunctionality>,
}

#[derive(DdsType, Debug, Clone)]
pub struct ConsumerDiscovery {
    #[dust_dds(key)]
    pub consumer_id: String,
    pub requested_functionality: ProvidedFunctionality,
}

#[derive(DdsType, Debug)]
pub struct EmptyMessage;

#[derive(DdsType, Debug)]
pub struct ProviderExchange<T: TypeSupport + XTypesSerialize + for <'a> XTypesDeserialize<'a> + Send>  {
    pub id: u32,
    pub payload: T,
}

impl<T: TypeSupport + XTypesSerialize + for <'a> XTypesDeserialize<'a> + Send> ProviderExchange<T> {
    pub fn new(id: u32, payload: T) -> Self {
        Self { id, payload }
    }
}