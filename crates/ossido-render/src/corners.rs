//! Angoli delle finestre: superellisse misurata su macOS 27 (`plan/reference/windows.md`,
//! "Finestre · forma e angoli"), applicata con uno shader alle superfici della finestra.

use smithay::backend::renderer::Frame;
use smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement;
use smithay::backend::renderer::element::utils::RescaleRenderElement;
use smithay::backend::renderer::element::{Element, Id, Kind, RenderElement, UnderlyingStorage};
use smithay::backend::renderer::gles::{
    GlesError, GlesFrame, GlesRenderer, GlesTexProgram, Uniform, UniformValue,
};
use smithay::backend::renderer::utils::{CommitCounter, DamageSet, OpaqueRegions};
use smithay::utils::{Buffer, Physical, Point, Rectangle, Scale, Size, Transform};

/// Superellisse |x/r|ⁿ + |y/r|ⁿ = 1 che riproduce la maschera degli angoli misurata
/// (errore quadratico 0,42 sui pixel mancanti; r e n sono correlati, la coppia non è
/// determinata). In punti logici.
pub const CORNER_RADIUS: f64 = 33.5;
pub const CORNER_EXPONENT: f32 = 4.5;

/// Il raggio in pixel fisici per una finestra di questa dimensione: non oltre metà del
/// lato più corto, perché i riquadri degli angoli non si sovrappongano.
pub fn radius_for(geometry: Size<f64, Physical>, scale: f64) -> f64 {
    (CORNER_RADIUS * scale)
        .min(geometry.w / 2.0)
        .min(geometry.h / 2.0)
        .max(0.0)
}

pub const SHADER: &str = include_str!("shaders/corners.frag");
pub const UNIFORMS: [(&str, smithay::backend::renderer::gles::UniformType); 5] = {
    use smithay::backend::renderer::gles::UniformType;
    [
        ("frag_to_local_x", UniformType::_3f),
        ("frag_to_local_y", UniformType::_3f),
        ("geo_size", UniformType::_2f),
        ("radius", UniformType::_1f),
        ("exponent", UniformType::_1f),
    ]
};

/// Le due righe della trasformazione affine da gl_FragCoord alla posizione nella finestra.
/// Il frame disegna l'output con `transform` (winit: Flipped180, cattura: Normal), quindi
/// le coordinate del framebuffer si riportano all'output con l'inversa.
fn frag_to_local(
    output: Size<i32, Physical>,
    window: Rectangle<f64, Physical>,
    transform: Transform,
) -> ([f32; 3], [f32; 3]) {
    let framebuffer = transform.transform_size(output.to_f64());
    let inverse = transform.invert();
    let map = |x: f64, y: f64| {
        inverse.transform_point_in(Point::<f64, Physical>::from((x, y)), &framebuffer)
    };
    let origin = map(0.0, 0.0);
    let along_x = map(1.0, 0.0) - origin;
    let along_y = map(0.0, 1.0) - origin;
    let offset = origin - window.loc;
    (
        [along_x.x as f32, along_y.x as f32, offset.x as f32],
        [along_x.y as f32, along_y.y as f32, offset.y as f32],
    )
}

/// Una superficie della finestra (principale o sottosuperficie), scalata attorno a un
/// punto durante le animazioni (scala 1 a riposo).
pub type WindowSurface = RescaleRenderElement<WaylandSurfaceRenderElement<GlesRenderer>>;

/// Una superficie della finestra con gli angoli tagliati. Senza programma (shader non
/// compilato) si disegna com'è.
pub struct RoundedSurface {
    inner: WindowSurface,
    /// La geometria della finestra nell'output, in pixel fisici.
    window: Rectangle<f64, Physical>,
    radius: f64,
    scale: f64,
    /// Dimensione dell'output in pixel fisici, prima della trasformazione del frame.
    output: Size<i32, Physical>,
    program: Option<GlesTexProgram>,
}

impl RoundedSurface {
    pub fn new(
        inner: WindowSurface,
        window: Rectangle<f64, Physical>,
        scale: f64,
        output: Size<i32, Physical>,
        program: Option<GlesTexProgram>,
    ) -> Self {
        let radius = radius_for(window.size, scale);
        Self {
            inner,
            window,
            radius,
            scale,
            output,
            program,
        }
    }

    /// `shift`: di quanto il frame sposta l'elemento rispetto all'output (cattura del vetro
    /// in una texture più piccola, sempre con trasformazione Normal).
    fn uniforms(&self, transform: Transform, shift: Point<i32, Physical>) -> Vec<Uniform<'static>> {
        let (mut x, mut y) = frag_to_local(self.output, self.window, transform);
        if transform == Transform::Normal {
            x[2] += shift.x as f32;
            y[2] += shift.y as f32;
        }
        vec![
            Uniform::new("frag_to_local_x", UniformValue::_3f(x[0], x[1], x[2])),
            Uniform::new("frag_to_local_y", UniformValue::_3f(y[0], y[1], y[2])),
            Uniform::new(
                "geo_size",
                UniformValue::_2f(self.window.size.w as f32, self.window.size.h as f32),
            ),
            Uniform::new("radius", UniformValue::_1f(self.radius as f32)),
            Uniform::new("exponent", UniformValue::_1f(CORNER_EXPONENT)),
        ]
    }

    /// I quattro riquadri degli angoli, relativi all'origine dell'elemento.
    fn corner_squares(&self, scale: Scale<f64>) -> [Rectangle<i32, Physical>; 4] {
        let origin = self.inner.geometry(scale).loc.to_f64();
        let local = self.window.loc - origin;
        let r = self.radius.ceil();
        let (w, h) = (self.window.size.w, self.window.size.h);
        let square = |x: f64, y: f64| {
            Rectangle::new(
                Point::from((x.floor() as i32, y.floor() as i32)),
                Size::from((r as i32 + 1, r as i32 + 1)),
            )
        };
        [
            square(local.x, local.y),
            square(local.x + w - r, local.y),
            square(local.x, local.y + h - r),
            square(local.x + w - r, local.y + h - r),
        ]
    }
}

impl Element for RoundedSurface {
    fn id(&self) -> &Id {
        self.inner.id()
    }

    fn current_commit(&self) -> CommitCounter {
        self.inner.current_commit()
    }

    fn src(&self) -> Rectangle<f64, Buffer> {
        self.inner.src()
    }

    fn transform(&self) -> Transform {
        self.inner.transform()
    }

    fn geometry(&self, scale: Scale<f64>) -> Rectangle<i32, Physical> {
        self.inner.geometry(scale)
    }

    fn location(&self, scale: Scale<f64>) -> Point<i32, Physical> {
        self.inner.location(scale)
    }

    fn damage_since(
        &self,
        scale: Scale<f64>,
        commit: Option<CommitCounter>,
    ) -> DamageSet<i32, Physical> {
        self.inner.damage_since(scale, commit)
    }

    /// Gli angoli tagliati non sono più opachi: si tolgono dalle regioni opache, o il
    /// damage tracker salterebbe quello che c'è sotto.
    fn opaque_regions(&self, scale: Scale<f64>) -> OpaqueRegions<i32, Physical> {
        let opaque = self.inner.opaque_regions(scale);
        if self.program.is_none() {
            return opaque;
        }
        let corners = self.corner_squares(scale);
        Rectangle::subtract_rects_many(opaque.iter().copied(), corners)
            .into_iter()
            .collect()
    }

    fn alpha(&self) -> f32 {
        self.inner.alpha()
    }

    fn kind(&self) -> Kind {
        self.inner.kind()
    }
}

impl RenderElement<GlesRenderer> for RoundedSurface {
    fn draw(
        &self,
        frame: &mut GlesFrame<'_, '_>,
        src: Rectangle<f64, Buffer>,
        dst: Rectangle<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
        opaque_regions: &[Rectangle<i32, Physical>],
    ) -> Result<(), GlesError> {
        let Some(program) = &self.program else {
            return self.inner.draw(frame, src, dst, damage, opaque_regions);
        };
        let shift = self.inner.geometry(Scale::from(self.scale)).loc - dst.loc;
        let uniforms = self.uniforms(frame.transformation(), shift);
        frame.override_default_tex_program(program.clone(), uniforms);
        let result = self.inner.draw(frame, src, dst, damage, opaque_regions);
        frame.clear_tex_program_override();
        result
    }

    fn underlying_storage(&self, renderer: &mut GlesRenderer) -> Option<UnderlyingStorage<'_>> {
        // Con gli angoli tagliati il buffer non si può mostrare così com'è (piani DRM).
        if self.program.is_some() {
            return None;
        }
        self.inner.underlying_storage(renderer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Applica la trasformazione dello shader a un punto del framebuffer.
    fn local(rows: ([f32; 3], [f32; 3]), x: f32, y: f32) -> (f32, f32) {
        let (rx, ry) = rows;
        (rx[0] * x + rx[1] * y + rx[2], ry[0] * x + ry[1] * y + ry[2])
    }

    #[test]
    fn fragment_maps_to_window_for_offscreen_and_winit_frames() {
        let window = Rectangle::new(Point::from((10.0, 5.0)), Size::from((40.0, 20.0)));
        let map = |transform| frag_to_local(Size::from((100, 50)), window, transform);
        // Cattura fuori schermo (Normal): il framebuffer coincide con l'output.
        assert_eq!(local(map(Transform::Normal), 10.5, 5.5), (0.5, 0.5));
        // Finestra winit (Flipped180): la riga 0 dell'output è in cima al framebuffer GL.
        assert_eq!(local(map(Transform::Flipped180), 10.5, 44.5), (0.5, 0.5));
    }

    #[test]
    fn radius_is_clamped_to_half_the_short_side() {
        assert_eq!(radius_for(Size::from((900.0, 600.0)), 1.0), 33.5);
        assert_eq!(radius_for(Size::from((900.0, 600.0)), 2.0), 67.0);
        assert_eq!(radius_for(Size::from((100.0, 40.0)), 1.0), 20.0);
        assert_eq!(radius_for(Size::from((0.0, 0.0)), 1.0), 0.0);
    }
}
