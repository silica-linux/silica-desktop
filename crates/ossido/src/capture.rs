//! Capture of the output into memory, for `ui.screenshot`.

use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::element::RenderElement;
use smithay::backend::renderer::{ExportMem, ImportAll, Offscreen, Renderer, Texture};
use smithay::desktop::space::space_render_elements;
use smithay::desktop::{Space, Window};
use smithay::output::Output;
use smithay::utils::{Rectangle, Transform};

/// An 8-bit RGBA frame, rows from top to bottom.
pub struct Capture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Capture {
    pub fn to_png(&self) -> Result<Vec<u8>, String> {
        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(&mut out, self.width, self.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
        writer
            .write_image_data(&self.rgba)
            .map_err(|e| e.to_string())?;
        writer.finish().map_err(|e| e.to_string())?;
        Ok(out)
    }
}

/// The given rectangle (pixels, rounded and clipped to the edges) of a capture; None if empty.
pub fn crop(capture: &Capture, x: f64, y: f64, width: f64, height: f64) -> Option<Capture> {
    if ![x, y, width, height].iter().all(|v| v.is_finite()) {
        return None;
    }
    let clamp = |v: f64, max: u32| v.round().clamp(0.0, f64::from(max)) as u32;
    let (x0, y0) = (clamp(x, capture.width), clamp(y, capture.height));
    let (x1, y1) = (
        clamp(x + width, capture.width),
        clamp(y + height, capture.height),
    );
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let (w, h) = (x1 - x0, y1 - y0);
    let stride = capture.width as usize * 4;
    let mut rgba = Vec::with_capacity(w as usize * h as usize * 4);
    for row in y0..y1 {
        let start = row as usize * stride + x0 as usize * 4;
        rgba.extend_from_slice(capture.rgba.get(start..start + w as usize * 4)?);
    }
    Some(Capture {
        width: w,
        height: h,
        rgba,
    })
}

/// Draws the space into an offscreen buffer and reads it back (renderer without effects).
pub fn capture<R, T>(
    renderer: &mut R,
    output: &Output,
    space: &Space<Window>,
    clear_color: [f32; 4],
) -> Result<Capture, String>
where
    R: Renderer + ImportAll + Offscreen<T> + ExportMem,
    R::TextureId: Clone + Texture + 'static,
{
    let elements = space_render_elements::<_, Window, _>(renderer, [space], output, 1.0)
        .map_err(|e| format!("{e:?}"))?;
    capture_elements(renderer, output, &elements, clear_color)
}

/// Draws the given elements into an offscreen buffer and reads it back.
pub fn capture_elements<R, T, E>(
    renderer: &mut R,
    output: &Output,
    elements: &[E],
    clear_color: [f32; 4],
) -> Result<Capture, String>
where
    R: Renderer + Offscreen<T> + ExportMem,
    E: RenderElement<R>,
{
    let mode = output.current_mode().ok_or("output without a mode")?;
    let size = mode.size;
    let width = u32::try_from(size.w).map_err(|_| "invalid width")?;
    let height = u32::try_from(size.h).map_err(|_| "invalid height")?;
    let buffer_size = size.to_logical(1).to_buffer(1, Transform::Normal);

    let mut target = renderer
        .create_buffer(Fourcc::Abgr8888, buffer_size)
        .map_err(|e| format!("offscreen buffer: {e:?}"))?;
    let mut framebuffer = renderer
        .bind(&mut target)
        .map_err(|e| format!("bind: {e:?}"))?;
    let scale = output.current_scale().fractional_scale();
    let mut damage_tracker = OutputDamageTracker::new(size, scale, Transform::Normal);
    damage_tracker
        .render_output(renderer, &mut framebuffer, 0, elements, clear_color)
        .map_err(|e| format!("render: {e:?}"))?;
    let mapping = renderer
        .copy_framebuffer(
            &framebuffer,
            Rectangle::from_size(buffer_size),
            Fourcc::Abgr8888,
        )
        .map_err(|e| format!("copy: {e:?}"))?;
    let bytes = renderer
        .map_texture(&mapping)
        .map_err(|e| format!("readback: {e:?}"))?;

    let stride = (width as usize).checked_mul(4).ok_or("frame too large")?;
    let expected = stride
        .checked_mul(height as usize)
        .ok_or("frame too large")?;
    if bytes.len() < expected {
        return Err(format!(
            "the renderer returned {} bytes, {expected} are needed",
            bytes.len()
        ));
    }
    let rgba = bytes.get(..expected).unwrap_or_default().to_vec();
    Ok(Capture {
        width,
        height,
        rgba,
    })
}
