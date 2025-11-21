mod example_messages;
mod consumer_impl;

use crate::example_messages::face_recognition::*;
use dust_dds::infrastructure::type_support::{DdsType};
use dust_dds::std_runtime::StdRuntime;
use mycellium_computing::core::application::Application;
use mycellium_computing::core::application::provider::ProviderTrait;
use mycellium_computing::{consumes, provides};
use std::env;
use dust_dds::dds_async::domain_participant_factory::DomainParticipantFactoryAsync;

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
        ModelsInfo {
            models: vec![]
        }
    }
}



async fn provider() {
    let mut app = Application::new(0, "JustASumService").await;

    app.register_provider::<FaceRecognition>().await;

    let topic = app.participant.create_topic::<PersonFrameData>(
        "person_in_frame",
        "PersonFrameData",
        dust_dds::infrastructure::qos::QosKind::Default,
        dust_dds::listener::NO_LISTENER,
        dust_dds::infrastructure::status::NO_STATUS,
    ).await.unwrap();

    let _writer = app.publisher.create_datawriter(
        &topic,
        dust_dds::infrastructure::qos::QosKind::Default,
        dust_dds::listener::NO_LISTENER,
        dust_dds::infrastructure::status::NO_STATUS,
    ).await.unwrap();

    for _i in 0..5 {
        println!("Sending person_in_frame data");
        FaceRecognition::person_in_frame(&_writer, &PersonFrameData {
            person_id: 1,
            distance: 1.5f32,
            sentiment: Prediction {
                label: "happy".to_string(),
                confidence: 0.95,
            },
        }).await;
    }

    app.run_forever().await;
}

async fn consumer() {
    let factory = DomainParticipantFactoryAsync::get_instance();

    let participant = factory.create_participant(
        0,
        dust_dds::infrastructure::qos::QosKind::Default,
        dust_dds::listener::NO_LISTENER,
        dust_dds::infrastructure::status::NO_STATUS,
    ).await.unwrap();

    let subscriber = participant.create_subscriber(
        dust_dds::infrastructure::qos::QosKind::Default,
        dust_dds::listener::NO_LISTENER,
        dust_dds::infrastructure::status::NO_STATUS,
    ).await.unwrap();

    let publisher = participant.create_publisher(
        dust_dds::infrastructure::qos::QosKind::Default,
        dust_dds::listener::NO_LISTENER,
        dust_dds::infrastructure::status::NO_STATUS,
    ).await.unwrap();

    let consumer = crate::consumer_impl::FaceRecognitionProxy::init(&participant, &subscriber, &publisher).await;

    // Consumer waiting forever

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        println!("Using as consumer");
        consumer().await;
    } else if args[1] == "provider" {
        println!("Using as provider");
        provider().await;
    }
}
