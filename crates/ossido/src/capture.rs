//! Cattura dell'output in memoria, per `ui.screenshot`.

use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::element::RenderElement;
use smithay::backend::renderer::{ExportMem, ImportAll, Offscreen, Renderer, Texture};
use smithay::desktop::space::space_render_elements;
use smithay::desktop::{Space, Window};
use smithay::output::Output;
use smithay::utils::{Rectangle, Transform};

use crate::state::CLEAR_COLOR;

/// Un frame in RGBA a 8 bit, righe dall'alto in basso.
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

/// Disegna lo spazio in un buffer fuori schermo e lo rilegge (renderer senza effetti).
pub fn capture<R, T>(
    renderer: &mut R,
    output: &Output,
    space: &Space<Window>,
) -> Result<Capture, String>
where
    R: Renderer + ImportAll + Offscreen<T> + ExportMem,
    R::TextureId: Clone + Texture + 'static,
{
    let elements = space_render_elements::<_, Window, _>(renderer, [space], output, 1.0)
        .map_err(|e| format!("{e:?}"))?;
    capture_elements(renderer, output, &elements)
}

/// Disegna gli elementi dati in un buffer fuori schermo e lo rilegge.
pub fn capture_elements<R, T, E>(
    renderer: &mut R,
    output: &Output,
    elements: &[E],
) -> Result<Capture, String>
where
    R: Renderer + Offscreen<T> + ExportMem,
    E: RenderElement<R>,
{
    let mode = output.current_mode().ok_or("output senza modo")?;
    let size = mode.size;
    let width = u32::try_from(size.w).map_err(|_| "larghezza non valida")?;
    let height = u32::try_from(size.h).map_err(|_| "altezza non valida")?;
    let buffer_size = size.to_logical(1).to_buffer(1, Transform::Normal);

    let mut target = renderer
        .create_buffer(Fourcc::Abgr8888, buffer_size)
        .map_err(|e| format!("buffer fuori schermo: {e:?}"))?;
    let mut framebuffer = renderer
        .bind(&mut target)
        .map_err(|e| format!("bind: {e:?}"))?;
    let scale = output.current_scale().fractional_scale();
    let mut damage_tracker = OutputDamageTracker::new(size, scale, Transform::Normal);
    damage_tracker
        .render_output(renderer, &mut framebuffer, 0, elements, CLEAR_COLOR)
        .map_err(|e| format!("render: {e:?}"))?;
    let mapping = renderer
        .copy_framebuffer(
            &framebuffer,
            Rectangle::from_size(buffer_size),
            Fourcc::Abgr8888,
        )
        .map_err(|e| format!("copia: {e:?}"))?;
    let bytes = renderer
        .map_texture(&mapping)
        .map_err(|e| format!("lettura: {e:?}"))?;

    let stride = (width as usize)
        .checked_mul(4)
        .ok_or("frame troppo grande")?;
    let expected = stride
        .checked_mul(height as usize)
        .ok_or("frame troppo grande")?;
    if bytes.len() < expected {
        return Err(format!(
            "il renderer ha restituito {} byte, ne servono {expected}",
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
