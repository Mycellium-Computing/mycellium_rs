mod example_messages;

use crate::example_messages::face_recognition::*;
use dust_dds::dds_async::domain_participant_factory::DomainParticipantFactoryAsync;
use mycelium_computing::core::application::Application;
use mycelium_computing::{consumes, provides};
use std::env;
// Import the desired DUST DDS runtime. By default, DUST DDS provides a standard implementation.
// The DUST DDS standard runtime depends on the std library. Then is not compatible with no_std.
use dust_dds::std_runtime::StdRuntime;

// TODO: Allow state via embassy_sync crate
// Creates a method "person_in_frame", which is used to publish a data piece when needed.
#[provides(StdRuntime, [
    RequestResponse("face_recognition", FaceRecognitionRequest, FaceRecognitionResponse),
    Response("available_models", ModelsInfo),
    Continuous("person_in_frame", PersonFrameData),
])]
struct FaceRecognition;

// Callbacks implementing the provider functionality
impl FaceRecognitionProviderTrait for FaceRecognition {
    async fn face_recognition(_input: FaceRecognitionRequest) -> FaceRecognitionResponse {
        FaceRecognitionResponse {
            model: "dummy".to_string(),
            applied: true,
            final_status: true,
        }
    }

    async fn available_models() -> ModelsInfo {
        ModelsInfo { models: vec![] }
    }
}

#[consumes(StdRuntime, [
    RequestResponse("face_recognition", FaceRecognitionRequest, FaceRecognitionResponse),
    Response("happy_face_recognition", FaceRecognitionResponse),
    Continuous("person_in_frame", PersonFrameData)
])]
struct FaceRecognitionProxy;

impl FaceRecognitionProxyContinuosTrait for FaceRecognitionProxy {
    async fn person_in_frame(data: PersonFrameData) {
        println!(
            "Person in frame: ID={}, Distance={}, Sentiment=({:?})",
            data.person_id, data.distance, data.sentiment
        );
    }
}

async fn provider() {
    let factory = DomainParticipantFactoryAsync::get_instance();
    let mut app = Application::new(0, "JustASumService", factory).await;

    app.register_provider::<FaceRecognition>().await;

    app.run_forever().await;
}

async fn consumer() {
    let factory = DomainParticipantFactoryAsync::get_instance();

    let participant = factory
        .create_participant(
            0,
            dust_dds::infrastructure::qos::QosKind::Default,
            dust_dds::listener::NO_LISTENER,
            dust_dds::infrastructure::status::NO_STATUS,
        )
        .await
        .unwrap();

    let subscriber = participant
        .create_subscriber(
            dust_dds::infrastructure::qos::QosKind::Default,
            dust_dds::listener::NO_LISTENER,
            dust_dds::infrastructure::status::NO_STATUS,
        )
        .await
        .unwrap();

    let publisher = participant
        .create_publisher(
            dust_dds::infrastructure::qos::QosKind::Default,
            dust_dds::listener::NO_LISTENER,
            dust_dds::infrastructure::status::NO_STATUS,
        )
        .await
        .unwrap();

    let consumer = FaceRecognitionProxy::init(&participant, &subscriber, &publisher).await;

    // Consumer waiting forever

    loop {
        let res = consumer
            .face_recognition(
                FaceRecognitionRequest {
                    model: "dummy".to_string(),
                    current_status: true,
                },
                dust_dds::dcps::infrastructure::time::Duration::new(10, 0),
            )
            .await;

        println!("Face recognition response: {:?}", res);
    }
}

async fn main_async() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        println!("Using as consumer");
        consumer().await;
    } else if args[1] == "provider" {
        println!("Using as provider");
        provider().await;
    }
}

fn main() {
    smol::block_on(main_async());
}
