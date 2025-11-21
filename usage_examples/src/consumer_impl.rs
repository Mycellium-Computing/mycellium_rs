
// Lets draft the Consumer here
// suppose this:
//
// #[consumes(StdRuntime, [
//     RequestResponse("face_recognition", FaceRecognitionRequest, FaceRecognitionResponse),
//     Response("happy_face_recognition", FaceRecognitionResponse),
//     Continuous("person_in_frame", PersonFrameData)
// ])]
// struct FaceRecognitionProxy;

use dust_dds::infrastructure::status::{StatusKind, NO_STATUS};
use dust_dds::std_runtime::StdRuntime; // Passed as runtime in the macro
use dust_dds::subscription::data_reader_listener::DataReaderListener;
use dust_dds::infrastructure::time::Duration;
use dust_dds::runtime::DdsRuntime;
use futures::{FutureExt, TryFutureExt};
use futures_timer::Delay;
use mycellium_computing::core::messages::{EmptyMessage, ProviderExchange};
use crate::example_messages::face_recognition::{FaceRecognitionRequest, FaceRecognitionResponse, PersonFrameData};

// Every non-continuous functionality generates a writer and a reader.
pub(crate) struct FaceRecognitionProxy {
    face_recognition_writer: dust_dds::dds_async::data_writer::DataWriterAsync<StdRuntime, ProviderExchange<FaceRecognitionRequest>>,
    face_recognition_reader: dust_dds::dds_async::data_reader::DataReaderAsync<StdRuntime, ProviderExchange<FaceRecognitionResponse>>,
    happy_face_recognition_writer: dust_dds::dds_async::data_writer::DataWriterAsync<StdRuntime, ProviderExchange<EmptyMessage>>,
    happy_face_recognition_reader: dust_dds::dds_async::data_reader::DataReaderAsync<StdRuntime, ProviderExchange<FaceRecognitionResponse>>,
}

// This is generated once per continuous functionality where the number is the index, in case that multiple functions return the same type
struct PersonFrameDataListener1;
impl DataReaderListener<StdRuntime, PersonFrameData> for PersonFrameDataListener1 {
    async fn on_data_available(&mut self, reader: dust_dds::dds_async::data_reader::DataReaderAsync<StdRuntime, PersonFrameData>) {
        let samples = reader
            .take(100, dust_dds::infrastructure::sample_info::ANY_SAMPLE_STATE, dust_dds::infrastructure::sample_info::ANY_VIEW_STATE, dust_dds::infrastructure::sample_info::ANY_INSTANCE_STATE)
            .await;

        if let Ok(data) = samples {
            for sample in &data {
                if let Ok(data) = sample.data() {
                    FaceRecognitionProxy::person_in_frame(data).await;
                }
            }
        }
    }
}

impl FaceRecognitionProxy {
    pub(crate) async fn init(
        participant: &dust_dds::dds_async::domain_participant::DomainParticipantAsync<StdRuntime>,
        subscriber: &dust_dds::dds_async::subscriber::SubscriberAsync<StdRuntime>,
        publisher: &dust_dds::dds_async::publisher::PublisherAsync<StdRuntime>,
    ) -> Self {
        // Continuous readers need a topic, then they need a participant and a subscriber
        let topic = participant.create_topic::<PersonFrameData>(
            "person_in_frame",
            "PersonFrameData",
            dust_dds::infrastructure::qos::QosKind::Default,
            dust_dds::listener::NO_LISTENER,
            dust_dds::infrastructure::status::NO_STATUS,
        ).await.unwrap();

        subscriber.create_datareader::<PersonFrameData>(
            &topic,
            dust_dds::infrastructure::qos::QosKind::Default,
            Some(PersonFrameDataListener1),
            &[StatusKind::DataAvailable],
        ).await.unwrap();

        // Create the topics, writers and readers for the non-continuous functionalities

        let face_recognition_req_topic = participant.create_topic::<ProviderExchange<FaceRecognitionRequest>>(
            "face_recognition_Req",
            "ProviderExchange<FaceRecognitionRequest>",
            dust_dds::infrastructure::qos::QosKind::Default,
            dust_dds::listener::NO_LISTENER,
            dust_dds::infrastructure::status::NO_STATUS,
        ).await.unwrap();

        let face_recognition_res_topic = participant.create_topic::<ProviderExchange<FaceRecognitionResponse>>(
            "face_recognition_Res",
            "ProviderExchange<FaceRecognitionResponse>",
            dust_dds::infrastructure::qos::QosKind::Default,
            dust_dds::listener::NO_LISTENER,
            dust_dds::infrastructure::status::NO_STATUS,
        ).await.unwrap();

        let face_recognition_writer = publisher.create_datawriter::<ProviderExchange<FaceRecognitionRequest>>(
            &face_recognition_req_topic,
            dust_dds::infrastructure::qos::QosKind::Default,
            dust_dds::listener::NO_LISTENER,
            dust_dds::infrastructure::status::NO_STATUS,
        ).await.unwrap();

        let face_recognition_reader = subscriber.create_datareader::<ProviderExchange<FaceRecognitionResponse>>(
            &face_recognition_res_topic,
            dust_dds::infrastructure::qos::QosKind::Default,
            dust_dds::listener::NO_LISTENER,
            dust_dds::infrastructure::status::NO_STATUS,
        ).await.unwrap();

        let happy_face_recognition_req_topic = participant.create_topic::<ProviderExchange<FaceRecognitionRequest>>(
            "happy_face_recognition_Req",
            "ProviderExchange<FaceRecognitionRequest>",
            dust_dds::infrastructure::qos::QosKind::Default,
            dust_dds::listener::NO_LISTENER,
            dust_dds::infrastructure::status::NO_STATUS,
        ).await.unwrap();

        let happy_face_recognition_res_topic = participant.create_topic::<ProviderExchange<FaceRecognitionResponse>>(
            "happy_face_recognition_Res",
            "ProviderExchange<FaceRecognitionResponse>",
            dust_dds::infrastructure::qos::QosKind::Default,
            dust_dds::listener::NO_LISTENER,
            dust_dds::infrastructure::status::NO_STATUS,
        ).await.unwrap();

        let happy_face_recognition_writer = publisher.create_datawriter::<ProviderExchange<EmptyMessage>>(
            &happy_face_recognition_req_topic,
            dust_dds::infrastructure::qos::QosKind::Default,
            dust_dds::listener::NO_LISTENER,
            dust_dds::infrastructure::status::NO_STATUS,
        ).await.unwrap();

        let happy_face_recognition_reader = subscriber.create_datareader::<ProviderExchange<FaceRecognitionResponse>>(
            &happy_face_recognition_res_topic,
            dust_dds::infrastructure::qos::QosKind::Default,
            dust_dds::listener::NO_LISTENER,
            dust_dds::infrastructure::status::NO_STATUS,
        ).await.unwrap();

        FaceRecognitionProxy {
            face_recognition_writer,
            face_recognition_reader,
            happy_face_recognition_writer,
            happy_face_recognition_reader,
        }
    }
}

// The trait for continuous functionalities
trait FaceRecognitionProxyContinuosTrait {
    async fn person_in_frame(data: PersonFrameData);
} // Must be implemented by the user
mod metrics {
    use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
    use std::sync::OnceLock;

    pub struct ThroughputMetrics {
        bytes_counter: AtomicU64,
        msg_counter: AtomicUsize,
        start_time: std::sync::Mutex<Option<std::time::Instant>>,
        reported: std::sync::atomic::AtomicBool,
    }

    impl ThroughputMetrics {
        fn new() -> Self {
            Self {
                bytes_counter: AtomicU64::new(0),
                msg_counter: AtomicUsize::new(0),
                start_time: std::sync::Mutex::new(None),
                reported: std::sync::atomic::AtomicBool::new(false),
            }
        }

        pub fn record(&self, bytes: u64) {
            self.bytes_counter.fetch_add(bytes, Ordering::Relaxed);
            self.msg_counter.fetch_add(1, Ordering::Relaxed);

            let mut start = self.start_time.lock().unwrap();
            if start.is_none() {
                *start = Some(std::time::Instant::now());
                println!("Starting throughput measurement...");
            } else if let Some(start_instant) = *start {
                let elapsed = start_instant.elapsed();
                if elapsed.as_secs() >= 60 && !self.reported.load(Ordering::Relaxed) {
                    let bytes = self.bytes_counter.load(Ordering::Relaxed);
                    let msgs = self.msg_counter.load(Ordering::Relaxed);
                    let bytes_per_sec = bytes as f64 / elapsed.as_secs_f64();
                    let msgs_per_sec = msgs as f64 / elapsed.as_secs_f64();

                    println!(
                        "1-minute Throughput: {:.2} KB/s, {:.2} msgs/s ({} msgs, {:.2} MB total)",
                        bytes_per_sec / 1024.0,
                        msgs_per_sec,
                        msgs,
                        bytes as f64 / (1024.0 * 1024.0)
                    );
                    self.reported.store(true, Ordering::Relaxed);
                }
            }
        }

        pub fn instance() -> &'static Self {
            static INSTANCE: OnceLock<ThroughputMetrics> = OnceLock::new();
            INSTANCE.get_or_init(ThroughputMetrics::new)
        }
    }
}


impl FaceRecognitionProxyContinuosTrait for FaceRecognitionProxy {
    async fn person_in_frame(data: PersonFrameData) {
        let data_size = std::mem::size_of_val(&data) as u64;
        metrics::ThroughputMetrics::instance().record(data_size);
    }
}


// The trait for response functionalities
trait FaceRecognitionProxyResponseTrait {
    async fn face_recognition(&self, data: FaceRecognitionRequest, timeout: Duration) -> Option<FaceRecognitionResponse>;
    async fn happy_face_recognition(&self, timeout: Duration) -> Option<FaceRecognitionResponse>;
}

impl FaceRecognitionProxyResponseTrait for FaceRecognitionProxy {
    async fn face_recognition(&self, data: FaceRecognitionRequest, timeout: Duration) -> Option<FaceRecognitionResponse> {
        let request = ProviderExchange {
            id: 0, // For now
            payload: data,
        };

        let (sender, receiver) = StdRuntime::oneshot::<FaceRecognitionResponse>();

        let listener = mycellium_computing::core::listener::ProviderResponseListener {
            expected_id: request.id,
            response_sender: Some(sender),
        };

        self.face_recognition_reader
            .set_listener(Some(listener), &[StatusKind::DataAvailable])
            .await
            .unwrap();

        self.face_recognition_writer
            .write(&request, None)
            .await
            .unwrap();

        let data_future = async { receiver.await.ok() }.fuse();

        let timer_future = Delay::new(core::time::Duration::new(timeout.sec() as u64, timeout.nanosec())).fuse();

        futures::pin_mut!(data_future);
        futures::pin_mut!(timer_future);

        futures::select! {
            res = data_future => res,
            _ = timer_future => None,
        }
    }

    async fn happy_face_recognition(&self, timeout: Duration) -> Option<FaceRecognitionResponse> {
        let request = ProviderExchange {
            id: 0, // For now
            payload: EmptyMessage,
        };

        let (sender, receiver) = StdRuntime::oneshot::<FaceRecognitionResponse>();

        let listener = mycellium_computing::core::listener::ProviderResponseListener {
            expected_id: request.id,
            response_sender: Some(sender),
        };

        self.happy_face_recognition_reader
            .set_listener(Some(listener), &[StatusKind::DataAvailable])
            .await
            .unwrap();

        self.happy_face_recognition_writer
            .write(&request, None)
            .await
            .unwrap();

        let data_future = async { receiver.await.ok() }.fuse();

        let timer_future = Delay::new(core::time::Duration::new(timeout.sec() as u64, timeout.nanosec())).fuse();

        futures::pin_mut!(data_future);
        futures::pin_mut!(timer_future);

        futures::select! {
            res = data_future => res,
            _ = timer_future => None,
        }
    }
}