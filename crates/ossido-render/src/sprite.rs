//! Piccoli elementi disegnati da Ossido sulla CPU: capsule, linee e testo in Inter, caricati
//! come texture e tenuti in cache finché non cambia il contenuto. Servono all'interfaccia che
//! il compositor disegna da sé (la barra degli spazi di Panorama), dove un client Flutter non
//! c'è.

use std::cell::RefCell;

use ab_glyph::{Font, FontRef, PxScale, ScaleFont, point};
use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::element::Kind;
use smithay::backend::renderer::element::texture::{TextureBuffer, TextureRenderElement};
use smithay::backend::renderer::gles::{GlesRenderer, GlesTexture};
use smithay::utils::{Logical, Point, Size, Transform};
use tracing::warn;

const INTER_REGULAR: &[u8] = include_bytes!("../fonts/Inter-Regular.ttf");
const INTER_SEMIBOLD: &[u8] = include_bytes!("../fonts/Inter-SemiBold.ttf");

/// Peso del testo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weight {
    Regular,
    SemiBold,
}

impl Weight {
    fn font(self) -> Option<FontRef<'static>> {
        FontRef::try_from_slice(match self {
            Weight::Regular => INTER_REGULAR,
            Weight::SemiBold => INTER_SEMIBOLD,
        })
        .ok()
    }
}

/// Larghezza di un testo in punti.
pub fn text_width(text: &str, size: f32, weight: Weight) -> f32 {
    let Some(font) = weight.font() else {
        return 0.0;
    };
    let scaled = font.as_scaled(PxScale::from(size));
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
}

/// Una tela RGBA premoltiplicata, in pixel; le coordinate dei metodi sono in punti.
pub struct Canvas {
    width: u32,
    height: u32,
    scale: f32,
    pixels: Vec<[f32; 4]>,
}

/// Distanza con segno da un rettangolo con angoli di raggio `r` (negativa dentro).
fn rounded_distance(px: f32, py: f32, rect: [f32; 4], r: f32) -> f32 {
    let [x, y, w, h] = rect;
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    let cx = x + w / 2.0;
    let cy = y + h / 2.0;
    let qx = (px - cx).abs() - (w / 2.0 - r);
    let qy = (py - cy).abs() - (h / 2.0 - r);
    let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
    outside + qx.max(qy).min(0.0) - r
}

impl Canvas {
    fn new(size: Size<f64, Logical>, scale: f32) -> Option<Self> {
        let width = (size.w as f32 * scale).ceil().max(1.0) as u32;
        let height = (size.h as f32 * scale).ceil().max(1.0) as u32;
        // Al più 4096 × 4096: un errore nei dati non deve allocare gigabyte.
        if width > 4096 || height > 4096 {
            return None;
        }
        Some(Self {
            width,
            height,
            scale,
            pixels: vec![[0.0; 4]; (width as usize) * (height as usize)],
        })
    }

    /// Copre il pixel con `color` (RGBA 0–255) per una copertura 0–1 ("over").
    fn blend(&mut self, x: u32, y: u32, color: [u8; 4], coverage: f32) {
        if coverage <= 0.0 || x >= self.width || y >= self.height {
            return;
        }
        let a = f32::from(color[3]) / 255.0 * coverage.min(1.0);
        let index = y as usize * self.width as usize + x as usize;
        if let Some(px) = self.pixels.get_mut(index) {
            for (i, channel) in px.iter_mut().take(3).enumerate() {
                let c = color.get(i).copied().map_or(0.0, f32::from) / 255.0;
                *channel = c * a + *channel * (1.0 - a);
            }
            px[3] = a + px[3] * (1.0 - a);
        }
    }

    /// Riempie un rettangolo arrotondato (punti).
    pub fn fill_rounded(&mut self, rect: [f32; 4], radius: f32, color: [u8; 4]) {
        self.shape(rect, radius, color, |d| (0.5 - d).clamp(0.0, 1.0));
    }

    /// Il contorno di `width` punti, centrato sul bordo del rettangolo.
    pub fn stroke_rounded(&mut self, rect: [f32; 4], radius: f32, width: f32, color: [u8; 4]) {
        let half = width * self.scale / 2.0;
        self.shape(rect, radius, color, |d| {
            (half + 0.5 - d.abs()).clamp(0.0, 1.0)
        });
    }

    fn shape(&mut self, rect: [f32; 4], radius: f32, color: [u8; 4], cover: impl Fn(f32) -> f32) {
        let s = self.scale;
        let rect_px = [rect[0] * s, rect[1] * s, rect[2] * s, rect[3] * s];
        let x0 = (rect_px[0] - 2.0).floor().max(0.0) as u32;
        let y0 = (rect_px[1] - 2.0).floor().max(0.0) as u32;
        let x1 = ((rect_px[0] + rect_px[2] + 2.0).ceil().max(0.0) as u32).min(self.width);
        let y1 = ((rect_px[1] + rect_px[3] + 2.0).ceil().max(0.0) as u32).min(self.height);
        for y in y0..y1 {
            for x in x0..x1 {
                let d = rounded_distance(x as f32 + 0.5, y as f32 + 0.5, rect_px, radius * s);
                self.blend(x, y, color, cover(d));
            }
        }
    }

    /// Una linea di `width` punti con le estremità arrotondate.
    pub fn line(&mut self, from: [f32; 2], to: [f32; 2], width: f32, color: [u8; 4]) {
        let s = self.scale;
        let (ax, ay, bx, by) = (from[0] * s, from[1] * s, to[0] * s, to[1] * s);
        let half = width * s / 2.0;
        let x0 = (ax.min(bx) - half - 2.0).floor().max(0.0) as u32;
        let y0 = (ay.min(by) - half - 2.0).floor().max(0.0) as u32;
        let x1 = ((ax.max(bx) + half + 2.0).ceil().max(0.0) as u32).min(self.width);
        let y1 = ((ay.max(by) + half + 2.0).ceil().max(0.0) as u32).min(self.height);
        let (dx, dy) = (bx - ax, by - ay);
        let len2 = (dx * dx + dy * dy).max(1e-6);
        for y in y0..y1 {
            for x in x0..x1 {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let t = (((px - ax) * dx + (py - ay) * dy) / len2).clamp(0.0, 1.0);
                let (cx, cy) = (ax + t * dx, ay + t * dy);
                let d = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt() - half;
                self.blend(x, y, color, (0.5 - d).clamp(0.0, 1.0));
            }
        }
    }

    /// Scrive `text` con la linea di base in `origin` (punti); restituisce la larghezza.
    pub fn text(
        &mut self,
        origin: [f32; 2],
        text: &str,
        size: f32,
        weight: Weight,
        color: [u8; 4],
    ) -> f32 {
        let Some(font) = weight.font() else {
            return 0.0;
        };
        let px = size * self.scale;
        let scaled = font.as_scaled(PxScale::from(px));
        let mut caret = origin[0] * self.scale;
        let baseline = origin[1] * self.scale;
        let start = caret;
        let mut previous = None;
        let mut glyphs = Vec::new();
        for c in text.chars() {
            let id = scaled.glyph_id(c);
            if let Some(previous) = previous {
                caret += scaled.kern(previous, id);
            }
            glyphs.push(id.with_scale_and_position(px, point(caret, baseline)));
            caret += scaled.h_advance(id);
            previous = Some(id);
        }
        for glyph in glyphs {
            let Some(outline) = font.outline_glyph(glyph) else {
                continue;
            };
            let bounds = outline.px_bounds();
            let mut cells = Vec::new();
            outline.draw(|x, y, c| cells.push((x, y, c)));
            for (x, y, c) in cells {
                let gx = bounds.min.x as i64 + i64::from(x);
                let gy = bounds.min.y as i64 + i64::from(y);
                if gx >= 0 && gy >= 0 {
                    self.blend(gx as u32, gy as u32, color, c);
                }
            }
        }
        (caret - start) / self.scale
    }

    fn rgba8(&self) -> Vec<u8> {
        self.pixels
            .iter()
            .flat_map(|px| px.map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8))
            .collect()
    }
}

/// Un elemento disegnato sulla tela, ridisegnato solo quando cambia la chiave.
#[derive(Default)]
pub struct Sprite {
    cached: RefCell<Option<(String, TextureBuffer<GlesTexture>)>>,
}

impl Sprite {
    /// L'elemento grande `size` punti in `location` (punti nell'output), disegnato da
    /// `draw` quando `key` cambia. `alpha` per le dissolvenze.
    #[allow(clippy::too_many_arguments)]
    pub fn element(
        &self,
        renderer: &mut GlesRenderer,
        key: &str,
        size: Size<f64, Logical>,
        location: Point<f64, Logical>,
        scale: f64,
        alpha: f32,
        draw: impl FnOnce(&mut Canvas),
    ) -> Option<TextureRenderElement<GlesTexture>> {
        let buffer_scale = scale.round().max(1.0) as i32;
        let full_key = format!("{key}@{buffer_scale}:{}x{}", size.w, size.h);
        let mut cached = self.cached.borrow_mut();
        if cached.as_ref().is_none_or(|(k, _)| *k != full_key) {
            let mut canvas = Canvas::new(size, buffer_scale as f32)?;
            draw(&mut canvas);
            let buffer_size = Size::<i32, smithay::utils::Buffer>::from((
                i32::try_from(canvas.width).ok()?,
                i32::try_from(canvas.height).ok()?,
            ));
            let buffer = match TextureBuffer::from_memory(
                renderer,
                &canvas.rgba8(),
                Fourcc::Abgr8888,
                buffer_size,
                false,
                buffer_scale,
                Transform::Normal,
                None,
            ) {
                Ok(buffer) => buffer,
                Err(err) => {
                    warn!("elemento disegnato non caricato: {err:?}");
                    return None;
                }
            };
            *cached = Some((full_key, buffer));
        }
        let (_, buffer) = cached.as_ref()?;
        Some(TextureRenderElement::from_texture_buffer(
            location
                .to_physical_precise_round::<f64, i32>(scale)
                .to_f64(),
            buffer,
            Some(alpha),
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
    fn shapes_and_text_cover_the_right_pixels() {
        let mut canvas = Canvas::new(Size::from((40.0, 20.0)), 1.0).unwrap();
        canvas.fill_rounded([0.0, 0.0, 40.0, 20.0], 6.0, [255, 0, 0, 255]);
        // Centro pieno, angolo vuoto.
        let at = |c: &Canvas, x: u32, y: u32| c.pixels[(y * c.width + x) as usize];
        assert_eq!(at(&canvas, 20, 10), [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(at(&canvas, 0, 0)[3], 0.0);
        let width = canvas.text(
            [2.0, 15.0],
            "Spazio 1",
            13.0,
            Weight::Regular,
            [0, 0, 0, 255],
        );
        assert!((width - text_width("Spazio 1", 13.0, Weight::Regular)).abs() < 0.01);
        assert!(width > 40.0);
        // Una tela enorme si rifiuta.
        assert!(Canvas::new(Size::from((10000.0, 10.0)), 1.0).is_none());
    }
}
