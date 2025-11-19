pub mod messages;

use dust_dds::dds_async::domain_participant::DomainParticipantAsync;
use dust_dds::dds_async::publisher::PublisherAsync;
use dust_dds::dds_async::subscriber::SubscriberAsync;
use dust_dds::std_runtime::StdRuntime;
use messages::*;
use mycellium_computing::core::application::messages::ProviderMessage;
use mycellium_computing::core::application::provider::ProviderTrait;
use std::time::Duration;

// #[consumes([
//     Continuous("pepper_front_camera", ImageData), // We look for a provider that gives continuous image data
//     Continuous("pepper_depth_camera", ImageDepthData), // We look something like a request-response provider
//     RequestResponse("vision_tools", CameraConfig, CameraConfig) // We look something like a request-response provider for image recognition
// ])]
// struct VisionUtilities;

// Generated struct instead:
struct VisionUtilities {
    pepper_front_camera_reader:
        dust_dds::dds_async::data_reader::DataReaderAsync<StdRuntime, ImageData>,
    pepper_depth_camera_reader:
        dust_dds::dds_async::data_reader::DataReaderAsync<StdRuntime, ImageDepthData>,
    vision_tools_request_writer:
        dust_dds::dds_async::data_writer::DataWriterAsync<StdRuntime, CameraConfig>,
    vision_tools_response_reader:
        dust_dds::dds_async::data_reader::DataReaderAsync<StdRuntime, CameraConfig>,
}

// hidden
impl VisionUtilities {
    async fn new(participant: DomainParticipantAsync<StdRuntime>) -> Self {
        // Initialization of the data topics and writers/readers would go here
        VisionUtilities {
            // Initialize readers/writers
            pepper_front_camera_reader: todo!(),
            pepper_depth_camera_reader: todo!(),
            vision_tools_request_writer: todo!(),
            vision_tools_response_reader: todo!(),
        }
    }

    async fn pepper_front_camera(&self) -> ImageData {
        // This function would read from the reader and return the ImageData.
        todo!()
    }

    async fn pepper_depth_camera(&self) -> ImageDepthData {
        // This function would read from the reader and return the ImageDepthData.
        todo!()
    }

    async fn vision_tools(&self, config: CameraConfig) -> CameraConfig {
        // This function would send a request and wait for the response.
        todo!()
    }
}

// #[provides(StdRuntime, [
//     RequestResponse("face_recognition", FaceRecognitionRequest, FaceRecognitionResponse),
//     RequestResponse("available_models", ModelsInfo),
//     Continuous("person_in_frame", PersonFrameData),
// ])]
// struct FaceRecognition;

// the following struct is generated instead:
struct FaceRecognition<S: Send + Sync> {
    person_in_frame_writer:
        dust_dds::dds_async::data_writer::DataWriterAsync<StdRuntime, PersonFrameData>,
    state: S,
}

// Desired flux:
// This application/module turns on the camera with the RequestResponse provider "vision_tools" from another module,
// continuously receives image and depth data from the camera providers, it implements face recognition using the received data,
// and provides the face recognition results and available models as request-response and continuous providers respectively.

// Hidden
trait FaceRecognitionReqResProvider<S: Send + Sync> {
    // Only implement the request-response functionalities
    async fn face_recognition(input: FaceRecognitionRequest, state: S) -> FaceRecognitionResponse;
    async fn available_models(state: S) -> ModelsInfo;
}

// Hidden
trait FaceRecognitionContinuousProvider {
    async fn person_in_frame(&self, data: &PersonFrameData);
}

// Hidden
impl<S: Send + Sync> FaceRecognitionContinuousProvider for FaceRecognition<S> {
    async fn person_in_frame(&self, data: &PersonFrameData) {
        self.person_in_frame_writer.write(data, None).await.unwrap();
    }
}

// Hidden
impl<S: Send + Sync> ProviderTrait for FaceRecognition<S> {
    fn get_functionalities() -> ProviderMessage {
        todo!()
    }

    async fn run_executor(
        tick_duration: Duration,
        functionality_name: String,
        participant: &DomainParticipantAsync<StdRuntime>,
        publisher: &PublisherAsync<StdRuntime>,
        subscriber: &SubscriberAsync<StdRuntime>,
    ) {
        todo!()
    }
}

// Hidden
impl<S: Send + Sync> FaceRecognition<S> {
    fn new(participant: DomainParticipantAsync<StdRuntime>, initial_state: S) -> Self {
        // Initialization of the data writers/readers would go here
        FaceRecognition {
            state: initial_state,
            person_in_frame_writer: todo!(),
        }
    }
}

struct MyAppState {
    dog: bool,
}

// FOR THE USER
impl FaceRecognitionReqResProvider<MyAppState> for FaceRecognition<MyAppState> {
    async fn face_recognition(
        input: FaceRecognitionRequest,
        state: MyAppState,
    ) -> FaceRecognitionResponse {
        todo!()
    }

    async fn available_models(state: MyAppState) -> ModelsInfo {
        todo!()
    }
}

async fn main() {}
