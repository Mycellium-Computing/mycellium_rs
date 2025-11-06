use dust_dds::infrastructure::type_support::DdsType;

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