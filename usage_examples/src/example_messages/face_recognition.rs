use dust_dds::infrastructure::type_support::DdsType;

#[derive(DdsType, Debug)]
pub struct Prediction {
    pub label: String,
    pub confidence: f32,
}

#[derive(DdsType, Debug)]
pub struct PersonFrameData {
    pub person_id: u32,
    pub distance: f32,
    pub sentiment: Prediction,
}

#[derive(DdsType, Debug)]
pub struct FaceRecognitionRequest {
    pub model: String,
    pub current_status: bool,
}

#[derive(DdsType, Debug)]
pub struct FaceRecognitionResponse {
    pub model: String,
    pub applied: bool,
    pub final_status: bool,
}

#[derive(DdsType, Debug)]
pub struct ModelInfo {
    pub name: String,
    pub accuracy: f32,
    pub description: String,
}

#[derive(DdsType, Debug)]
pub struct ModelsInfo {
    pub models: Vec<ModelInfo>,
}
