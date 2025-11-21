use crate::core::messages::{ConsumerDiscovery, ProviderMessage};
use dust_dds::dds_async::data_reader::DataReaderAsync;
use dust_dds::dds_async::data_writer::DataWriterAsync;
use dust_dds::infrastructure::sample_info::{ANY_INSTANCE_STATE, ANY_SAMPLE_STATE, ANY_VIEW_STATE};
use dust_dds::infrastructure::type_support::{DdsDeserialize, DdsSerialize, TypeSupport};
use dust_dds::std_runtime::StdRuntime;
use dust_dds::subscription::data_reader_listener::DataReaderListener;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

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
