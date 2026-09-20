use base64::Engine;
use xcap::Monitor;

/// Grab the whole primary monitor as a PNG data URL. The main widget is
/// content-protected, so it is excluded from this frame automatically.
pub fn grab_primary() -> Result<String, String> {
    let monitors = Monitor::all().map_err(|e| e.to_string())?;
    let monitor = monitors
        .iter()
        .find(|m| m.is_primary().unwrap_or(false))
        .or_else(|| monitors.first())
        .ok_or("No monitor found")?;

    let image = monitor.capture_image().map_err(|e| e.to_string())?;
    let w = image.width();
    let h = image.height();
    let raw = image.into_raw();

    let mut png_buf = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png_buf, w, h);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
        writer.write_image_data(&raw).map_err(|e| e.to_string())?;
    }

    let b64 = base64::engine::general_purpose::STANDARD.encode(&png_buf);
    Ok(format!("data:image/png;base64,{b64}"))
}
