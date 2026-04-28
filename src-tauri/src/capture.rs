use std::{fs, path::PathBuf, process::Command};

use anyhow::{anyhow, Context, Result};
use chrono::Local;
use image::{imageops::FilterType, ImageBuffer, ImageFormat, Rgb};

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

        if std::env::var("MINDBACK_SIMULATE_CAPTURE").as_deref() == Ok("1") {
            write_simulated_capture(&image_path)?;
            return Ok(CaptureResult {
                thumb_relative_path,
                image_path,
            });
        }

        let raw_path = day_dir.join(format!("capture-input-{timestamp}.png"));
        let output = Command::new("/usr/sbin/screencapture")
            .args(["-x", raw_path.to_string_lossy().as_ref()])
            .output()
            .context("failed to launch macOS screencapture")?;

        if !output.status.success() {
            return Err(anyhow!(
                "macOS screencapture failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let source = image::open(&raw_path)
            .with_context(|| format!("failed to open capture {}", raw_path.display()))?;
        let resized = source.resize(1280, 1280, FilterType::Lanczos3).to_rgb8();
        resized.save_with_format(&image_path, ImageFormat::Jpeg)?;
        let _ = fs::remove_file(raw_path);

        Ok(CaptureResult {
            thumb_relative_path,
            image_path,
        })
    }
}

fn write_simulated_capture(image_path: &PathBuf) -> Result<()> {
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

    image.save(image_path)?;
    Ok(())
}
