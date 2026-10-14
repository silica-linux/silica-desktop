//! Il titolo nella barra delle decorazioni lato server.
//!
//! Misure della barra degli strumenti (`plan/reference/windows.md`): altezza delle
//! maiuscole 10,75 pt → Inter a ~15 pt (inferenza: il font di sistema non si identifica),
//! peso semibold, linea di base a ~31 pt dalla cima, `#4d4d4d` attiva e `#bebebe` inattiva;
//! in scuro `#e9e9e9` e `#646464`.
//! In Finder il titolo segue i pulsanti di navigazione; una finestra con il solo titolo
//! non è misurata: qui è centrato, provvisorio.

use std::cell::RefCell;

use ab_glyph::{Font, FontRef, PxScale, ScaleFont, point};
use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::element::Kind;
use smithay::backend::renderer::element::texture::{TextureBuffer, TextureRenderElement};
use smithay::backend::renderer::gles::{GlesRenderer, GlesTexture};
use smithay::utils::{Logical, Point, Rectangle, Size, Transform};
use tracing::warn;

const INTER_SEMIBOLD: &[u8] = include_bytes!("../fonts/Inter-SemiBold.ttf");

/// Corpo del titolo in punti.
pub const SIZE: f32 = 15.0;
/// Linea di base dalla cima della cornice, in punti.
pub const BASELINE: f64 = 31.0;
/// Margine dal gruppo dei pulsanti (che finisce a x 80) e dal bordo destro.
const MARGIN: f64 = 92.0;
const ACTIVE: [u8; 3] = [0x4d, 0x4d, 0x4d];
const INACTIVE: [u8; 3] = [0xbe, 0xbe, 0xbe];
const ACTIVE_DARK: [u8; 3] = [0xe9, 0xe9, 0xe9];
const INACTIVE_DARK: [u8; 3] = [0x64, 0x64, 0x64];

/// Il colore del titolo secondo stato e aspetto.
pub fn color(active: bool, dark: bool) -> [u8; 3] {
    match (active, dark) {
        (true, false) => ACTIVE,
        (false, false) => INACTIVE,
        (true, true) => ACTIVE_DARK,
        (false, true) => INACTIVE_DARK,
    }
}

/// Il testo rasterizzato, in RGBA premoltiplicato.
struct Raster {
    width: u32,
    height: u32,
    /// Distanza della linea di base dalla cima del raster, in pixel.
    ascent: f32,
    pixels: Vec<u8>,
}

/// Rasterizza `text` a `px` pixel nel colore dato, al più `max_width` pixel: oltre, il
/// testo si accorcia con i puntini.
fn rasterize(text: &str, px: f32, color: [u8; 3], max_width: f32) -> Option<Raster> {
    let font = FontRef::try_from_slice(INTER_SEMIBOLD).ok()?;
    let scaled = font.as_scaled(PxScale::from(px));
    let advance = |text: &str| {
        let mut width = 0.0;
        let mut previous = None;
        for c in text.chars() {
            let id = scaled.glyph_id(c);
            if let Some(previous) = previous {
                width += scaled.kern(previous, id);
            }
            width += scaled.h_advance(id);
            previous = Some(id);
        }
        width
    };
    let mut shown = text.to_owned();
    if advance(&shown) > max_width {
        let mut chars: Vec<char> = text.chars().collect();
        while !chars.is_empty() {
            chars.pop();
            let candidate: String = chars.iter().collect::<String>().trim_end().to_owned() + "…";
            if advance(&candidate) <= max_width {
                shown = candidate;
                break;
            }
        }
        if chars.is_empty() {
            return None;
        }
    }
    let ascent = scaled.ascent();
    let width = advance(&shown).ceil().max(1.0) as u32;
    let height = (ascent - scaled.descent()).ceil().max(1.0) as u32;
    let mut coverage = vec![0.0_f32; (width as usize).checked_mul(height as usize)?];
    let mut caret = 0.0;
    let mut previous = None;
    for c in shown.chars() {
        let id = scaled.glyph_id(c);
        if let Some(previous) = previous {
            caret += scaled.kern(previous, id);
        }
        let glyph = id.with_scale_and_position(px, point(caret, ascent));
        caret += scaled.h_advance(id);
        previous = Some(id);
        let Some(outline) = font.outline_glyph(glyph) else {
            continue;
        };
        let bounds = outline.px_bounds();
        outline.draw(|x, y, c| {
            let gx = bounds.min.x as i64 + i64::from(x);
            let gy = bounds.min.y as i64 + i64::from(y);
            if gx >= 0 && gy >= 0 && (gx as u32) < width && (gy as u32) < height {
                let index = gy as usize * width as usize + gx as usize;
                if let Some(cell) = coverage.get_mut(index) {
                    *cell = (*cell + c).min(1.0);
                }
            }
        });
    }
    let pixels = coverage
        .iter()
        .flat_map(|&c| {
            let a = (c * 255.0).round();
            let channel = |v: u8| (f32::from(v) * c).round() as u8;
            [
                channel(color[0]),
                channel(color[1]),
                channel(color[2]),
                a as u8,
            ]
        })
        .collect();
    Some(Raster {
        width,
        height,
        ascent,
        pixels,
    })
}

/// Il titolo di una finestra, tenuto nei suoi dati: si rasterizza solo quando cambiano
/// testo, stato, scala o larghezza disponibile.
#[derive(Default)]
pub struct Title {
    state: RefCell<Option<Cached>>,
}

struct Cached {
    key: (String, [u8; 3], u32, i32),
    buffer: TextureBuffer<GlesTexture>,
    /// Posizione della linea di base nel raster, in punti.
    ascent: f64,
    size: Size<i32, Logical>,
}

impl Title {
    /// L'elemento del titolo per una cornice (logica, nell'output). Niente se il testo è
    /// vuoto o non c'è spazio.
    pub fn element(
        &self,
        renderer: &mut GlesRenderer,
        text: &str,
        frame: Rectangle<i32, Logical>,
        color: [u8; 3],
        scale: f64,
    ) -> Option<TextureRenderElement<GlesTexture>> {
        let text = text.trim();
        if text.is_empty() {
            return None;
        }
        let buffer_scale = scale.round().max(1.0) as i32;
        let available = f64::from(frame.size.w) - 2.0 * MARGIN;
        if available < f64::from(SIZE) {
            return None;
        }
        let key = (
            text.to_owned(),
            color,
            buffer_scale.unsigned_abs(),
            frame.size.w,
        );
        let mut state = self.state.borrow_mut();
        if state.as_ref().is_none_or(|cached| cached.key != key) {
            let px = SIZE * buffer_scale as f32;
            let raster = rasterize(
                text,
                px,
                color,
                (available * f64::from(buffer_scale)) as f32,
            )?;
            let size = Size::<i32, smithay::utils::Buffer>::from((
                i32::try_from(raster.width).ok()?,
                i32::try_from(raster.height).ok()?,
            ));
            let buffer = match TextureBuffer::from_memory(
                renderer,
                &raster.pixels,
                Fourcc::Abgr8888,
                size,
                false,
                buffer_scale,
                Transform::Normal,
                None,
            ) {
                Ok(buffer) => buffer,
                Err(err) => {
                    warn!("titolo non caricato: {err:?}");
                    return None;
                }
            };
            *state = Some(Cached {
                key,
                buffer,
                ascent: f64::from(raster.ascent) / f64::from(buffer_scale),
                size: size.to_logical(buffer_scale, Transform::Normal),
            });
        }
        let cached = state.as_ref()?;
        // Centrato nella cornice, senza toccare i pulsanti.
        let x = (f64::from(frame.loc.x)
            + (f64::from(frame.size.w) - f64::from(cached.size.w)) / 2.0)
            .max(f64::from(frame.loc.x) + MARGIN);
        let y = f64::from(frame.loc.y) + BASELINE - cached.ascent;
        let location: Point<i32, smithay::utils::Physical> =
            Point::<f64, Logical>::from((x, y)).to_physical_precise_round(scale);
        Some(TextureRenderElement::from_texture_buffer(
            location.to_f64(),
            &cached.buffer,
            None,
            None,
            None,
            Kind::Unspecified,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_rasterize_and_shorten_to_fit() {
        let raster = rasterize("Documenti", 15.0, ACTIVE, 1000.0).unwrap();
        assert!(
            raster.width > 40 && raster.height >= 15,
            "{}x{}",
            raster.width,
            raster.height
        );
        // Qualche pixel pieno: il testo c'è.
        assert!(raster.pixels.chunks(4).any(|p| p[3] > 200));
        let short = rasterize("Un titolo molto lungo per la barra", 15.0, ACTIVE, 80.0).unwrap();
        assert!(short.width <= 80, "{}", short.width);
        assert!(rasterize("Titolo", 15.0, ACTIVE, 1.0).is_none());
    }
}
