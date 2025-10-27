// PLACEHOLDER MAIN FILE FOR DEVELOPMENT PURPOSES.
// THIS FILE WILL NOT BE PART OF THE FINAL LIBRARY CRATE.
mod core;

use std::any::Any;
use std::env;
use dust_dds::{
    infrastructure::{type_support::DdsType},
};
use primitives::{provides};

use crate::core::application::Application;
use crate::core::application::messages::{ProviderMessage, ProvidedFunctionality};
use crate::core::application::provider::ProviderTrait;

#[derive(DdsType)]
struct Image {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

#[derive(DdsType)]
struct BBox {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    class_id: u32,
    confidence: f32,
}

#[derive(DdsType)]
struct ImagePrediction {
    results: Vec<BBox>,
}

#[derive(Default)]
#[provides([
    ("face_detection", Image, ImagePrediction),
    ("coco_detection", Image, ImagePrediction),
])]
struct VisualObjectDetection;

// This is what the user must do to implement the provider methods

impl VisualObjectDetectionProviderTrait for VisualObjectDetection {
    fn face_detection(&self, input: Image) -> ImagePrediction {
        todo!()
    }

    fn coco_detection(&self, input: Image) -> ImagePrediction {
        todo!()
    }
}

fn main() {
    let domain_id = match env::var("DDS_DOMAIN_ID") {
        Ok(res) => res.parse::<u32>().unwrap(),
        Err(_) => 0u32
    };
    let mut app = Application::new(domain_id, "ExampleApp");

    app.register_provider::<VisualObjectDetection>();

    app.run(); // This method will lock the thread and run the event loop
}
