// PLACEHOLDER MAIN FILE FOR DEVELOPMENT PURPOSES.
// THIS FILE WILL NOT BE PART OF THE FINAL LIBRARY CRATE.
mod core;

use dust_dds::{
    infrastructure::{type_support::DdsType},
};
use primitives::{Provider, provides};

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

#[derive(Provider)]
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
}
