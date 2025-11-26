use crate::core::messages::ProviderExchange;
use core::pin::Pin;
use dust_dds::dds_async::data_reader::DataReaderAsync;
use dust_dds::dds_async::data_writer::DataWriterAsync;
use dust_dds::infrastructure::sample_info::{ANY_INSTANCE_STATE, ANY_SAMPLE_STATE, ANY_VIEW_STATE};
use dust_dds::infrastructure::type_support::{DdsDeserialize, DdsSerialize, TypeSupport};
use dust_dds::runtime::DdsRuntime;
use dust_dds::std_runtime::oneshot::OneshotSender;
use dust_dds::subscription::data_reader_listener::DataReaderListener;
use dust_dds::xtypes::deserialize::XTypesDeserialize;
use dust_dds::xtypes::serialize::XTypesSerialize;

pub struct RequestListener<I, O, R: DdsRuntime> {
    pub writer: DataWriterAsync<R, O>,
    pub implementation: Box<dyn Fn(I) -> Pin<Box<dyn Future<Output = O> + Send>> + Send + Sync>,
}

impl<I, O, R: DdsRuntime> DataReaderListener<R, I> for RequestListener<I, O, R>
where
    I: for<'a> DdsDeserialize<'a> + Send + Sync + 'static,
    O: DdsSerialize + Send + Sync + 'static,
{
    async fn on_data_available(&mut self, reader: DataReaderAsync<R, I>) {
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

pub struct ProviderResponseListener<T: Send> {
    pub expected_id: u32,
    pub response_sender: Option<OneshotSender<T>>,
}

impl<T, R: DdsRuntime> DataReaderListener<R, ProviderExchange<T>> for ProviderResponseListener<T>
where
    T: TypeSupport + XTypesSerialize + for<'a> XTypesDeserialize<'a> + Send + Sync + 'static,
{
    async fn on_data_available(&mut self, reader: DataReaderAsync<R, ProviderExchange<T>>) {
        let samples = reader
            .take(100, ANY_SAMPLE_STATE, ANY_VIEW_STATE, ANY_INSTANCE_STATE)
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
