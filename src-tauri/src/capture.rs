use std::path::PathBuf;

use anyhow::Result;
use chrono::Local;
use image::{ImageBuffer, Rgb};

pub struct CaptureResult {
    pub thumb_relative_path: String,
    pub image_path: PathBuf,
}

pub struct CaptureService;

impl CaptureService {
    pub fn capture_once(day_dir: PathBuf) -> Result<CaptureResult> {
        let timestamp = Local::now().format("%H-%M-%S").to_string();
        let thumb_relative_path = format!("thumbs/{timestamp}.jpg");
        let image_path = day_dir.join(&thumb_relative_path);

        let mut image: ImageBuffer<Rgb<u8>, Vec<u8>> =
            ImageBuffer::from_pixel(1280, 720, Rgb([244u8, 246, 248]));
        for (x, y, pixel) in image.enumerate_pixels_mut() {
            if x < 12 || y < 12 || x > 1268 || y > 708 {
                *pixel = Rgb([55u8, 65, 81]);
            }
            if (80..1200).contains(&x) && (120..170).contains(&y) {
                *pixel = Rgb([209u8, 220, 232]);
            }
            if (80..900).contains(&x) && (220..260).contains(&y) {
                *pixel = Rgb([225u8, 232, 240]);
            }
            if (80..1080).contains(&x) && (300..520).contains(&y) {
                *pixel = Rgb([235u8, 239, 244]);
            }
        }

        image.save(&image_path)?;

        Ok(CaptureResult {
            thumb_relative_path,
            image_path,
        })
    }
}
