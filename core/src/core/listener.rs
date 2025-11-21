use crate::core::messages::{ConsumerDiscovery, ProviderExchange, ProviderMessage};
use dust_dds::dds_async::data_reader::DataReaderAsync;
use dust_dds::dds_async::data_writer::DataWriterAsync;
use dust_dds::infrastructure::sample_info::{ANY_INSTANCE_STATE, ANY_SAMPLE_STATE, ANY_VIEW_STATE};
use dust_dds::infrastructure::type_support::{DdsDeserialize, DdsSerialize, TypeSupport};
use dust_dds::std_runtime::StdRuntime;
use dust_dds::subscription::data_reader_listener::DataReaderListener;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use dust_dds::std_runtime::oneshot::OneshotSender;
use dust_dds::xtypes::deserialize::XTypesDeserialize;
use dust_dds::xtypes::serialize::XTypesSerialize;

pub struct RequestListener<T, R> {
    pub writer: DataWriterAsync<StdRuntime, R>,
    pub implementation: Box<dyn Fn(T) -> Pin<Box<dyn Future<Output = R> + Send>> + Send + Sync>,
}

impl<T, R> DataReaderListener<StdRuntime, T> for RequestListener<T, R>
where
    T: for<'a> DdsDeserialize<'a> + Send + Sync + 'static,
    R: DdsSerialize + Send + Sync + 'static,
{
    async fn on_data_available(&mut self, reader: DataReaderAsync<StdRuntime, T>) {
        let samples = reader
            .take(100, ANY_SAMPLE_STATE, ANY_VIEW_STATE, ANY_INSTANCE_STATE)
            .await;

        if let Ok(data) = samples {
            for sample in &data {
                if let Ok(data) = sample.data() {
                    let result = (self.implementation)(data).await;
                    self.writer.write(&result, None).await.unwrap();
                }
            }
        }
    }
}

pub struct ConsumerAppearListener {
    pub providers: Arc<Mutex<Vec<ProviderMessage>>>,
    pub writer: Arc<DataWriterAsync<StdRuntime, ProviderMessage>>,
}
impl DataReaderListener<StdRuntime, ConsumerDiscovery> for ConsumerAppearListener {
    async fn on_data_available(&mut self, reader: DataReaderAsync<StdRuntime, ConsumerDiscovery>) {
        let samples = reader
            .take(1, ANY_SAMPLE_STATE, ANY_VIEW_STATE, ANY_INSTANCE_STATE)
            .await;

        if let Ok(data) = samples {
            for sample in data {
                if let Ok(_) = sample.data() {
                    let providers = {
                        let lock = self.providers.lock().unwrap();
                        lock.clone()
                    };
                    for provider in providers {
                        self.writer.write(&provider, None).await.unwrap();
                    }
                }
            }
        }
    }
}

pub struct ProviderResponseListener<T: Send>
{
    pub expected_id: u32,
    pub response_sender: Option<OneshotSender<T>>
}

impl<T> DataReaderListener<StdRuntime, ProviderExchange<T>> for ProviderResponseListener<T>
where
    T: TypeSupport + XTypesSerialize + for <'a> XTypesDeserialize<'a> + Send + Sync + 'static,
{
    async fn on_data_available(&mut self, reader: DataReaderAsync<StdRuntime, ProviderExchange<T>>) {
        let samples = reader
            .take(1, ANY_SAMPLE_STATE, ANY_VIEW_STATE, ANY_INSTANCE_STATE)
            .await;

        if let Err(_) = samples {
            return;
        }

        let samples = samples.unwrap();

        let found = samples
            .iter()
            .filter_map(|sample| {
                if let Ok(data) = sample.data() {
                    if data.id == self.expected_id {
                        return Some(data);
                    }
                }
                None
            })
            .next();

        if let Some(data) = found {
            if let Some(sender) = self.response_sender.take() {
                sender.send(data.payload);
            }
        };
    }
}
