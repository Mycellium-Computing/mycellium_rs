use dust_dds::infrastructure::type_support::DdsType;

#[derive(DdsType)]
pub struct CameraConfig {
    pub resolution: u8,
    pub frame_rate: u8,
    pub color_mode: u8,
    pub saturation: u8,
    pub brightness: u8,
    pub contrast: u8,
    pub exposure: u8,
}

#[derive(DdsType)]
pub struct ImageData {
    pub width: u32,
    pub height: u32,
    pub data: Vec<[u8; 3]>, // RGB pixel data
}

#[derive(DdsType)]
pub struct ImageDepthData {
    pub width: u32,
    pub height: u32,
    pub data: Vec<f32>, // Depth values in meters
}

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

#[derive(DdsType)]
pub struct FaceRecognitionRequest {
    pub model: String,
    pub current_status: bool,
}

#[derive(DdsType)]
pub struct FaceRecognitionResponse {
    pub model: String,
    pub applied: bool,
    pub final_status: bool,
}


#[derive(DdsType)]
pub struct ModelInfo {
    pub name: String,
    pub accuracy: f32,
    pub description: String,
}

#[derive(DdsType)]
pub struct ModelsInfo {
    pub models: Vec<ModelInfo>,
}