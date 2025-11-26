use nokhwa::Camera;
use nokhwa::pixel_format::RgbFormat;
use nokhwa::utils::{
    CameraFormat, CameraIndex, FrameFormat, RequestedFormat, RequestedFormatType, Resolution,
};
use std::time::{Duration, Instant};

fn main() -> Result<(), nokhwa::NokhwaError> {
    let requested = RequestedFormat::new::<RgbFormat>(RequestedFormatType::Closest(
        CameraFormat::new(Resolution::new(640, 480), FrameFormat::MJPEG, 30),
    ));

    let mut camera = Camera::new(CameraIndex::Index(0), requested)?;

    camera.open_stream()?;

    // Capture frames for 10 seconds then exit
    let start = Instant::now();
    let duration = Duration::from_secs(10);

    loop {
        let frame = camera.frame()?;
        let resolution = frame.resolution();
        let frame_data = frame.buffer();
        println!(
            "Captured frame with dimensions: {}x{} size {} bytes",
            resolution.width(),
            resolution.height(),
            frame_data.len(),
        );

        if start.elapsed() >= duration {
            break;
        }
    }

    camera.stop_stream()?;
    Ok(())
}
