use dust_dds::infrastructure::type_support::DdsType;
use mycellium_computing::{
    core::application::Application,
    provides,
};

#[derive(DdsType)]
struct Image {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

#[derive(DdsType)]
struct FaceDetection {
    detected_face: bool,
}

#[derive(DdsType)]
struct ChatMessage {
    message: String,
}

#[derive(Default)]
#[provides([
    ("face_detection", Image, FaceDetection)
])]
struct VisualObjectDetection;

#[derive(Default)]
#[provides([
    ("chat", ChatMessage, ChatMessage)
])]
struct Chatbot;

impl VisualObjectDetectionProviderTrait for VisualObjectDetection {
    fn face_detection(&self, _input: Image) -> FaceDetection {
        todo!()
    }
}

impl ChatbotProviderTrait for Chatbot {
    fn chat(&self, input: ChatMessage) -> ChatMessage {
        ChatMessage {
            message: format!("Echo: {}", input.message),
        }
    }
}

fn main() {
    let mut app = Application::new(0, "ExampleApp");

    app.register_provider::<VisualObjectDetection>();
    app.register_provider::<Chatbot>();

    app.run();
}