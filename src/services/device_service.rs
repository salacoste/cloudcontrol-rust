use crate::device::adb::Adb;
use crate::device::atx_client::AtxClient;
use crate::utils::hierarchy;
use base64::Engine;
use serde_json::Value;

/// High-level device operations wrapping AtxClient.
/// Replaces Python `AndroidDevice` class.
pub struct DeviceService;

impl DeviceService {
    /// Take a screenshot and return base64-encoded JPEG.
    /// Uses spawn_blocking for CPU-bound image processing (Story 12-5).
    pub async fn screenshot_base64(
        client: &AtxClient,
        quality: u8,
        scale: f64,
    ) -> Result<String, String> {
        let jpeg_bytes = client.screenshot().await?;

        // If scale < 1.0, resize the image (non-blocking)
        let output_bytes = if scale < 1.0 || quality < 95 {
            Self::resize_jpeg_async(jpeg_bytes, quality, scale).await?
        } else {
            jpeg_bytes
        };

        Ok(base64::engine::general_purpose::STANDARD.encode(&output_bytes))
    }

    /// Take a screenshot and return raw JPEG bytes.
    /// Uses spawn_blocking for CPU-bound image processing (Story 12-5).
    pub async fn screenshot_jpeg(
        client: &AtxClient,
        quality: u8,
        scale: f64,
    ) -> Result<Vec<u8>, String> {
        let jpeg_bytes = client.screenshot().await?;

        if scale < 1.0 || quality < 95 {
            Self::resize_jpeg_async(jpeg_bytes, quality, scale).await
        } else {
            Ok(jpeg_bytes)
        }
    }

    /// USB-optimized screenshot: uses `adb exec-out screencap -p` directly.
    /// Returns base64-encoded JPEG. Uses spawn_blocking (Story 12-5).
    pub async fn screenshot_usb_base64(
        serial: &str,
        quality: u8,
        scale: f64,
    ) -> Result<String, String> {
        let png_bytes = Adb::screencap(serial).await?;
        let jpeg_bytes = Self::resize_jpeg_async(png_bytes, quality, scale).await?;
        Ok(base64::engine::general_purpose::STANDARD.encode(&jpeg_bytes))
    }

    /// USB-optimized screenshot: returns raw JPEG bytes with timing breakdown.
    /// Uses spawn_blocking for CPU-bound image processing (Story 12-5).
    pub async fn screenshot_usb_jpeg(
        serial: &str,
        quality: u8,
        scale: f64,
    ) -> Result<Vec<u8>, String> {
        let t0 = std::time::Instant::now();
        let png_bytes = Adb::screencap(serial).await?;
        let t_screencap = t0.elapsed();
        let png_kb = png_bytes.len() / 1024; // Capture before move

        let t1 = std::time::Instant::now();
        let jpeg_bytes = Self::resize_jpeg_async(png_bytes, quality, scale).await?;
        let t_convert = t1.elapsed();

        tracing::info!(
            "[Screenshot] USB q={} s={:.1} | screencap={:.0}ms ({}KB PNG) | convert={:.0}ms ({}KB JPEG) | total={:.0}ms",
            quality, scale,
            t_screencap.as_secs_f64() * 1000.0,
            png_kb,
            t_convert.as_secs_f64() * 1000.0,
            jpeg_bytes.len() / 1024,
            t0.elapsed().as_secs_f64() * 1000.0,
        );

        Ok(jpeg_bytes)
    }

    /// Non-blocking wrapper for resize_jpeg using spawn_blocking (Story 12-5).
    /// Image processing is CPU-bound and would otherwise block the async runtime.
    pub async fn resize_jpeg_async(
        data: Vec<u8>,
        quality: u8,
        scale: f64,
    ) -> Result<Vec<u8>, String> {
        let input_size = data.len();
        let start = std::time::Instant::now();

        let result = tokio::task::spawn_blocking(move || Self::resize_jpeg_blocking(&data, quality, scale))
            .await
            .map_err(|e| format!("spawn_blocking error: {}", e))?;

        let elapsed = start.elapsed();
        tracing::debug!(
            "[resize_jpeg_async] spawn_blocking completed: {}KB → {:?} output in {:.1}ms",
            input_size / 1024,
            result.as_ref().map(|r| format!("{}KB", r.len() / 1024)).unwrap_or_else(|e| e.clone()),
            elapsed.as_secs_f64() * 1000.0
        );

        result
    }

    /// Resize and recompress image data (PNG or JPEG input → JPEG output).
    /// Uses Nearest filter for maximum speed (matches Python's resample=0).
    /// This is a blocking operation - use resize_jpeg_async for async context.
    fn resize_jpeg_blocking(data: &[u8], quality: u8, scale: f64) -> Result<Vec<u8>, String> {
        let img = image::load_from_memory(data)
            .map_err(|e| format!("Failed to decode image: {}", e))?;

        let img = if scale < 1.0 {
            let new_w = (img.width() as f64 * scale) as u32;
            let new_h = (img.height() as f64 * scale) as u32;
            // Use Nearest for maximum speed (like Python's resample=0)
            img.resize(new_w, new_h, image::imageops::FilterType::Nearest)
        } else {
            img
        };

        let rgb = img.to_rgb8();
        let mut buf = std::io::Cursor::new(Vec::new());
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality);
        encoder
            .encode_image(&rgb)
            .map_err(|e| format!("JPEG encode failed: {}", e))?;

        Ok(buf.into_inner())
    }

    /// Convert raw image bytes (PNG/JPEG) to base64-encoded JPEG string.
    /// Used as fallback when AtxClient screenshot fails.
    /// Note: This is synchronous - caller should wrap in spawn_blocking if needed.
    pub fn encode_screenshot(raw_bytes: &[u8], quality: u8, scale: f64) -> Result<String, String> {
        let jpeg_bytes = Self::resize_jpeg_blocking(raw_bytes, quality, scale)?;
        Ok(base64::engine::general_purpose::STANDARD.encode(&jpeg_bytes))
    }

    /// Convert raw image bytes (PNG/JPEG) to JPEG bytes.
    /// Used as fallback when AtxClient screenshot fails.
    /// Note: This is synchronous - caller should wrap in spawn_blocking if needed.
    pub fn raw_screenshot_to_jpeg(raw_bytes: &[u8], quality: u8, scale: f64) -> Result<Vec<u8>, String> {
        Self::resize_jpeg_blocking(raw_bytes, quality, scale)
    }

    /// Dump and parse UI hierarchy to JSON.
    pub async fn dump_hierarchy(client: &AtxClient) -> Result<Value, String> {
        let xml = client.dump_hierarchy().await?;
        hierarchy::xml_to_json(&xml)
    }
}
