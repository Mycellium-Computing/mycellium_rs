mod example_messages;

use crate::example_messages::face_recognition::*;
use dust_dds::dds_async::domain_participant_factory::DomainParticipantFactoryAsync;
use mycellium_computing::core::application::Application;
use mycellium_computing::{consumes, provides};
use smol::Timer;
use std::env;
// TODO: Import in the macros
// Required imports
use dust_dds::runtime::DdsRuntime;
use futures::FutureExt;
// Import the desired DUST DDS runtime. By default, DUST DDS provides a standard implementation.
// The DUST DDS
use dust_dds::std_runtime::StdRuntime;

// TODO: Allow state.
// TODO:  (Backlog) completely decouple from StdRuntime to allow other runtimes.(To be implemented by the user)
// Creates a method "person_in_frame", which is used to publish a data piece when needed.
#[provides(StdRuntime, [
    RequestResponse("face_recognition", FaceRecognitionRequest, FaceRecognitionResponse),
    Response("available_models", ModelsInfo),
    Continuous("person_in_frame", PersonFrameData),
])]
struct FaceRecognition;

// Callbacks implementing the provider functionality
impl FaceRecognitionProviderTrait for FaceRecognition {
    async fn face_recognition(input: FaceRecognitionRequest) -> FaceRecognitionResponse {
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
    let mut app = Application::new(0, "JustASumService").await;

    app.register_provider::<FaceRecognition>().await;

    let sleep_fn = async |duration: core::time::Duration| {
        Timer::after(duration).await;
    };

    app.run_forever(sleep_fn).await;
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
