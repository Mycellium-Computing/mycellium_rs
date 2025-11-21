mod example_messages;

use crate::example_messages::face_recognition::*;
use dust_dds::infrastructure::type_support::{DdsType};
use dust_dds::std_runtime::StdRuntime;
use mycellium_computing::core::application::Application;
use mycellium_computing::{consumes, provides};
use std::env;

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
        todo!()
    }

    async fn available_models() -> ModelsInfo {
        todo!()
    }
}

//#[consumes([("add_two_ints", CalculatorRequest, Number)])]
struct CalculatorProxy;

async fn provider() {
    let mut app = Application::new(0, "JustASumService").await;

    app.register_provider::<FaceRecognition>().await;

    app.run_forever().await;
}

async fn consumer() {}

#[derive(DdsType)]
struct FaceRecognitionResponse {
    recognized_faces: Vec<String>,
    xd: bool,
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 1 {
        consumer().await;
    } else if args[0] == "provider" {
        provider().await;
    }
}
