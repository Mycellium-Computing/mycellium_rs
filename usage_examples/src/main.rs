mod consumer_impl;
mod discoveries_and_topics_qos;

use crate::consumer_impl::messages::*;
use dust_dds::infrastructure::type_support::{DdsType};
use dust_dds::std_runtime::StdRuntime;
use mycellium_computing::core::application::Application;
use mycellium_computing::{consumes, provides};
use std::env;
use std::time::Duration;

const HERTZ: u32 = 360;

#[provides(StdRuntime, [
    RequestResponse("face_recognition", FaceRecognitionRequest, FaceRecognitionResponse),
    Response("available_models", ModelsInfo),
    Continuous("person_in_frame", PersonFrameData),
])]
struct FaceRecognition;

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
    let tick_duration = Duration::from_nanos((1_000_000_000 / HERTZ) as u64);
    let mut app = Application::new(0, "JustASumService", tick_duration).await;

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
