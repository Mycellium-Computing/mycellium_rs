use dust_dds::infrastructure::type_support::DdsType;

#[derive(DdsType)]
pub struct CameraConfig {
    resolution: u8,
    frame_rate: u8,
    color_mode: u8,
    saturation: u8,
    brightness: u8,
    contrast: u8,
    exposure: u8,
}

#[derive(DdsType)]
pub struct ImageData {
    width: u32,
    height: u32,
    data: Vec<[u8; 3]>, // RGB pixel data
}

#[derive(DdsType)]
pub struct ImageDepthData {
    width: u32,
    height: u32,
    data: Vec<f32>, // Depth values in meters
}

#[derive(DdsType)]
pub struct Prediction {
    label: String,
    confidence: f32,
}

#[derive(DdsType)]
pub struct PersonFrameData {
    person_id: u32,
    distance: f32,
    sentiment: Prediction,
}

#[derive(DdsType)]
pub struct FaceRecognitionRequest {
    model: String,
    current_status: bool,
}

#[derive(DdsType)]
pub struct FaceRecognitionResponse {
    model: String,
    applied: bool,
    final_status: bool,
}


#[derive(DdsType)]
pub struct ModelInfo {
    name: String,
    accuracy: f32,
    description: String,
}

#[derive(DdsType)]
pub struct ModelsInfo {
    models: Vec<ModelInfo>,
}