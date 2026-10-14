//! L'effetto "genio" della riduzione nel Dock e del ripristino (`plan/reference/windows.md`,
//! "Riduzione nel Dock e ripristino"; curve in [`crate::animation::genie_minimize`]).
//!
//! L'istantanea della finestra si deforma in un imbuto verso la miniatura: a ogni riga dello
//! schermo i bordi passano dalla larghezza della finestra a quella della miniatura con una
//! smoothstep fra la cima della finestra e la cima della miniatura (misurata sui fotogrammi),
//! mentre il bordo superiore e quello inferiore del contenuto scendono ognuno con la sua curva.
//! Lo shader campiona l'istantanea al contrario: per ogni pixel dell'area dell'effetto trova il
//! punto della cornice che ci finisce.

use smithay::backend::renderer::Frame;
use smithay::backend::renderer::element::texture::TextureRenderElement;
use smithay::backend::renderer::element::{Element, Id, Kind, RenderElement};
use smithay::backend::renderer::gles::{
    GlesError, GlesFrame, GlesRenderer, GlesTexProgram, GlesTexture, Uniform, UniformType,
    UniformValue,
};
use smithay::backend::renderer::utils::{CommitCounter, OpaqueRegions};
use smithay::utils::{Buffer, Physical, Point, Rectangle, Scale, Size, Transform};

use crate::animation::GenieShape;

pub const SHADER: &str = include_str!("shaders/genie.frag");
pub const UNIFORMS: [(&str, UniformType); 6] = [
    ("frag_to_local_x", UniformType::_3f),
    ("frag_to_local_y", UniformType::_3f),
    ("window_rect", UniformType::_4f),
    ("target_rect", UniformType::_4f),
    ("motion", UniformType::_3f),
    ("frame_uv", UniformType::_4f),
];

/// Un genio nel frame: l'istantanea, dove sta la cornice, la miniatura e la forma corrente.
#[derive(Debug)]
pub struct GenieElement {
    id: Id,
    commit: CommitCounter,
    inner: TextureRenderElement<GlesTexture>,
    /// L'area dell'effetto nell'output: cornice e miniatura insieme.
    area: Rectangle<i32, Physical>,
    window: Rectangle<f64, Physical>,
    target: Rectangle<f64, Physical>,
    frame_uv: [f32; 4],
    shape: GenieShape,
    output: Size<i32, Physical>,
    program: Option<GlesTexProgram>,
}

/// I dati di un genio per [`GenieElement::new`].
pub struct GenieParts {
    pub id: Id,
    pub commit: CommitCounter,
    pub context_id: smithay::backend::renderer::ContextId<GlesTexture>,
    pub texture: GlesTexture,
    /// Dove sta l'istantanea nell'output, in pixel fisici.
    pub snapshot: Rectangle<i32, Physical>,
    /// La cornice della finestra (senza ombra) nell'output.
    pub window: Rectangle<f64, Physical>,
    /// La miniatura di destinazione nell'output.
    pub target: Rectangle<f64, Physical>,
    pub shape: GenieShape,
    pub output: Size<i32, Physical>,
    pub program: Option<GlesTexProgram>,
}

impl GenieElement {
    /// None se l'area è vuota o fuori dall'output.
    pub fn new(parts: GenieParts) -> Option<Self> {
        let union = parts.window.merge(parts.target);
        let area = Rectangle::new(
            Point::from((union.loc.x.floor() as i32, union.loc.y.floor() as i32)),
            Size::from((
                (union.size.w.ceil() as i32).saturating_add(2),
                (union.size.h.ceil() as i32).saturating_add(2),
            )),
        )
        .intersection(Rectangle::from_size(parts.output))?;
        if area.is_empty() || parts.snapshot.is_empty() {
            return None;
        }
        // La cornice dentro l'istantanea, in coordinate della texture (0–1, dall'alto).
        let snap = parts.snapshot.to_f64();
        let mut frame_uv = [
            ((parts.window.loc.x - snap.loc.x) / snap.size.w) as f32,
            ((parts.window.loc.y - snap.loc.y) / snap.size.h) as f32,
            (parts.window.size.w / snap.size.w) as f32,
            (parts.window.size.h / snap.size.h) as f32,
        ];
        if parts.texture.is_y_inverted() {
            frame_uv[1] = 1.0 - frame_uv[1];
            frame_uv[3] = -frame_uv[3];
        }
        let inner = TextureRenderElement::from_static_texture(
            Id::new(),
            parts.context_id,
            area.loc.to_f64(),
            parts.texture,
            1,
            Transform::Normal,
            None,
            None,
            Some(area.size.to_logical(1)),
            None,
            Kind::Unspecified,
        );
        let local = |r: Rectangle<f64, Physical>| Rectangle::new(r.loc - area.loc.to_f64(), r.size);
        Some(Self {
            id: parts.id,
            commit: parts.commit,
            inner,
            area,
            window: local(parts.window),
            target: local(parts.target),
            frame_uv,
            shape: parts.shape,
            output: parts.output,
            program: parts.program,
        })
    }

    fn uniforms(&self, transform: Transform, shift: Point<i32, Physical>) -> Vec<Uniform<'static>> {
        let (mut x, mut y) =
            crate::corners::frag_to_local(self.output, self.area.to_f64(), transform);
        if transform == Transform::Normal {
            x[2] += shift.x as f32;
            y[2] += shift.y as f32;
        }
        let rect = |r: Rectangle<f64, Physical>| {
            UniformValue::_4f(
                r.loc.x as f32,
                r.loc.y as f32,
                (r.loc.x + r.size.w) as f32,
                (r.loc.y + r.size.h) as f32,
            )
        };
        vec![
            Uniform::new("frag_to_local_x", UniformValue::_3f(x[0], x[1], x[2])),
            Uniform::new("frag_to_local_y", UniformValue::_3f(y[0], y[1], y[2])),
            Uniform::new("window_rect", rect(self.window)),
            Uniform::new("target_rect", rect(self.target)),
            Uniform::new(
                "motion",
                UniformValue::_3f(
                    self.shape.bend as f32,
                    self.shape.top as f32,
                    self.shape.bottom as f32,
                ),
            ),
            Uniform::new(
                "frame_uv",
                UniformValue::_4f(
                    self.frame_uv[0],
                    self.frame_uv[1],
                    self.frame_uv[2],
                    self.frame_uv[3],
                ),
            ),
        ]
    }
}

impl Element for GenieElement {
    fn id(&self) -> &Id {
        &self.id
    }

    fn current_commit(&self) -> CommitCounter {
        self.commit
    }

    fn src(&self) -> Rectangle<f64, Buffer> {
        self.inner.src()
    }

    fn geometry(&self, _scale: Scale<f64>) -> Rectangle<i32, Physical> {
        self.area
    }

    fn opaque_regions(&self, _scale: Scale<f64>) -> OpaqueRegions<i32, Physical> {
        OpaqueRegions::default()
    }

    fn kind(&self) -> Kind {
        Kind::Unspecified
    }
}

impl RenderElement<GlesRenderer> for GenieElement {
    fn draw(
        &self,
        frame: &mut GlesFrame<'_, '_>,
        src: Rectangle<f64, Buffer>,
        dst: Rectangle<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
        opaque_regions: &[Rectangle<i32, Physical>],
    ) -> Result<(), GlesError> {
        // Senza lo shader niente genio: la finestra sparisce subito (mai una texture stirata).
        let Some(program) = &self.program else {
            return Ok(());
        };
        // Il frame può disegnare l'area spostata (offscreen): la trasformazione segue `dst`.
        let shift = self.area.loc - dst.loc;
        let uniforms = self.uniforms(frame.transformation(), shift);
        frame.override_default_tex_program(program.clone(), uniforms);
        let result = RenderElement::<GlesRenderer>::draw(
            &self.inner,
            frame,
            src,
            dst,
            damage,
            opaque_regions,
        );
        frame.clear_tex_program_override();
        result
    }
}
