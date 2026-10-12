//! Vetro: lo sfondo dietro una superficie, sfocato e trasformato nel colore del
//! materiale, disegnato nelle regioni dichiarate con `silica-glass-v1`.
//!
//! Per ogni superficie con vetro: si ridisegnano gli elementi che le stanno dietro in una
//! texture fuori schermo (regioni più il margine della sfocatura), si sfoca con una
//! gaussiana separabile, si mescola la quota nitida misurata, e un `GlassElement` per
//! regione la disegna con la matrice di colore e gli angoli a superellisse. Le texture si
//! riusano finché lo sfondo non cambia. Parametri da `plan/reference/glass.md` e
//! `menubar.md` (aspetto chiaro).

use std::cell::RefCell;

use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::element::{Element, Id, Kind, RenderElement};
use smithay::backend::renderer::gles::{
    GlesError, GlesFrame, GlesRenderer, GlesTexProgram, GlesTexture, Uniform, UniformType,
    UniformValue,
};
use smithay::backend::renderer::utils::CommitCounter;
use smithay::backend::renderer::{Bind, Frame, Offscreen, Renderer};
use smithay::utils::{Buffer, Physical, Point, Rectangle, Scale, Size, Transform};

use crate::corners::CORNER_EXPONENT;

/// Parametri di un materiale: sfocatura, quota nitida e colore (`out = M · in + b` sui
/// valori sRGB codificati, 0–1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MaterialParams {
    /// σ della gaussiana in punti.
    pub sigma: f64,
    /// Quota dell'immagine nitida mescolata a quella sfocata.
    pub sharp: f32,
    pub matrix: [[f32; 3]; 3],
    pub offset: [f32; 3],
}

/// Fondo del Dock, chiaro: σ 2,5–3,25 pt (si usa 2,75), w 0,28, matrice libera
/// (rmse 2,45) con b = (37,3; 39,3; 36,8).
pub const CLEAR_LIGHT: MaterialParams = MaterialParams {
    sigma: 2.75,
    sharp: 0.28,
    matrix: [
        [1.100, -0.314, -0.033],
        [-0.146, 0.805, 0.087],
        [-0.158, -0.328, 1.242],
    ],
    offset: [37.3 / 255.0, 39.3 / 255.0, 36.8 / 255.0],
};

/// Menu, chiaro: σ ≈ 9 pt, componente nitida trascurabile, matrice (rmse 4,1) con
/// b = (142, 140, 140).
pub const THICK_LIGHT: MaterialParams = MaterialParams {
    sigma: 9.0,
    sharp: 0.0,
    matrix: [
        [0.781, -0.200, -0.124],
        [-0.079, 0.579, -0.034],
        [-0.166, -0.263, 0.899],
    ],
    offset: [142.0 / 255.0, 140.0 / 255.0, 140.0 / 255.0],
};

pub const BLUR_SHADER: &str = include_str!("shaders/blur.frag");
pub const BLUR_UNIFORMS: [(&str, UniformType); 2] =
    [("step", UniformType::_2f), ("sigma", UniformType::_1f)];
pub const MATERIAL_SHADER: &str = include_str!("shaders/material.frag");
pub const MATERIAL_UNIFORMS: [(&str, UniformType); 8] = [
    ("tex_size", UniformType::_2f),
    ("region", UniformType::_4f),
    ("radius", UniformType::_1f),
    ("exponent", UniformType::_1f),
    ("row_r", UniformType::_3f),
    ("row_g", UniformType::_3f),
    ("row_b", UniformType::_3f),
    ("offset", UniformType::_3f),
];

/// Oltre questo σ (pixel) si sfoca a metà o a un quarto della risoluzione: il nucleo
/// dello shader arriva a 48 pixel.
const MAX_SIGMA_PX: f64 = 12.0;

/// Una regione di vetro nell'output, in pixel fisici.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhysicalRegion {
    pub rect: Rectangle<i32, Physical>,
    pub radius: f64,
    pub material: MaterialParams,
}

/// Lo stato degli elementi dietro il vetro: se non cambia, la texture si riusa.
type Signature = Vec<(Id, CommitCounter, Rectangle<i32, Physical>)>;

/// Il vetro di una superficie fra un frame e l'altro (nei dati della superficie).
#[derive(Default)]
pub struct GlassCache {
    state: RefCell<Option<Cached>>,
}

struct Cached {
    signature: Signature,
    regions: Vec<PhysicalRegion>,
    bbox: Rectangle<i32, Physical>,
    /// Una texture sfocata per materiale presente, nell'ordine delle regioni.
    textures: Vec<(MaterialParams, GlesTexture)>,
    ids: Vec<Id>,
    commit: CommitCounter,
}

/// I programmi del vetro.
#[derive(Clone)]
pub struct GlassPrograms {
    pub blur: GlesTexProgram,
    pub material: GlesTexProgram,
}

impl GlassCache {
    /// Gli elementi del vetro per queste regioni, dato quello che sta dietro (`behind`,
    /// dal primo piano allo sfondo). Errore se il renderer non riesce a disegnare fuori
    /// schermo: il chiamante salta il vetro per questo frame.
    #[allow(clippy::too_many_arguments)]
    pub fn elements<E: RenderElement<GlesRenderer>>(
        &self,
        renderer: &mut GlesRenderer,
        programs: &GlassPrograms,
        regions: &[PhysicalRegion],
        behind: &[E],
        output: Rectangle<i32, Physical>,
        scale: f64,
        clear_color: [f32; 4],
    ) -> Result<Vec<GlassElement>, GlesError> {
        let margin = regions
            .iter()
            .map(|region| (region.material.sigma * scale * 3.0).ceil() as i32 + 2)
            .max()
            .unwrap_or(0);
        let Some(bbox) = regions
            .iter()
            .map(|region| region.rect)
            .reduce(|a, b| a.merge(b))
            .map(|rect| {
                Rectangle::new(
                    rect.loc - Point::from((margin, margin)),
                    rect.size + Size::from((margin.saturating_mul(2), margin.saturating_mul(2))),
                )
            })
            .and_then(|rect| rect.intersection(output))
        else {
            return Ok(Vec::new());
        };
        let signature: Signature = behind
            .iter()
            .filter(|element| element.geometry(Scale::from(scale)).overlaps(bbox))
            .map(|element| {
                (
                    element.id().clone(),
                    element.current_commit(),
                    element.geometry(Scale::from(scale)),
                )
            })
            .collect();

        let mut state = self.state.borrow_mut();
        let reuse = state.as_ref().is_some_and(|cached| {
            cached.signature == signature && cached.regions == regions && cached.bbox == bbox
        });
        if !reuse {
            let sharp = capture(renderer, behind, bbox, scale, clear_color)?;
            let mut textures: Vec<(MaterialParams, GlesTexture)> = Vec::new();
            for region in regions {
                if textures
                    .iter()
                    .any(|(material, _)| *material == region.material)
                {
                    continue;
                }
                let blurred = blur(
                    renderer,
                    programs,
                    &sharp,
                    bbox.size,
                    region.material,
                    scale,
                )?;
                textures.push((region.material, blurred));
            }
            let (ids, mut commit) = match state.take() {
                Some(cached) if cached.ids.len() == regions.len() => (cached.ids, cached.commit),
                Some(cached) => (regions.iter().map(|_| Id::new()).collect(), cached.commit),
                None => (
                    regions.iter().map(|_| Id::new()).collect(),
                    CommitCounter::default(),
                ),
            };
            commit.increment();
            *state = Some(Cached {
                signature,
                regions: regions.to_vec(),
                bbox,
                textures,
                ids,
                commit,
            });
        }
        let Some(cached) = state.as_ref() else {
            return Ok(Vec::new());
        };
        Ok(cached
            .regions
            .iter()
            .zip(&cached.ids)
            .filter_map(|(region, id)| {
                let texture = cached
                    .textures
                    .iter()
                    .find(|(material, _)| *material == region.material)?
                    .1
                    .clone();
                Some(GlassElement {
                    id: id.clone(),
                    commit: cached.commit,
                    texture,
                    bbox: cached.bbox,
                    region: *region,
                    program: programs.material.clone(),
                })
            })
            .collect())
    }
}

fn buffer_size(size: Size<i32, Physical>) -> Size<i32, Buffer> {
    size.to_logical(1).to_buffer(1, Transform::Normal)
}

/// Ridisegna gli elementi dietro il vetro (e il colore del desktop) dentro `bbox`.
fn capture<E: RenderElement<GlesRenderer>>(
    renderer: &mut GlesRenderer,
    behind: &[E],
    bbox: Rectangle<i32, Physical>,
    scale: f64,
    clear_color: [f32; 4],
) -> Result<GlesTexture, GlesError> {
    let mut texture = renderer.create_buffer(Fourcc::Abgr8888, buffer_size(bbox.size))?;
    {
        let mut target = renderer.bind(&mut texture)?;
        let mut frame = renderer.render(&mut target, bbox.size, Transform::Normal)?;
        frame.clear(clear_color.into(), &[Rectangle::from_size(bbox.size)])?;
        for element in behind.iter().rev() {
            let geometry = element.geometry(Scale::from(scale));
            let Some(visible) = geometry.intersection(bbox) else {
                continue;
            };
            let dst = Rectangle::new(geometry.loc - bbox.loc, geometry.size);
            let damage = [Rectangle::new(visible.loc - geometry.loc, visible.size)];
            element.draw(&mut frame, element.src(), dst, &damage, &[])?;
        }
        // Stesso contesto GL per chi userà la texture: l'ordine dei comandi basta.
        let _sync = frame.finish()?;
    }
    Ok(texture)
}

/// Una passata: `source` (grande `from`) disegnata in una texture nuova grande `to`,
/// sopra `under` se c'è.
#[allow(clippy::too_many_arguments)]
fn pass(
    renderer: &mut GlesRenderer,
    source: &GlesTexture,
    from: Size<i32, Physical>,
    to: Size<i32, Physical>,
    program: Option<&GlesTexProgram>,
    uniforms: &[Uniform<'_>],
    alpha: f32,
    under: Option<&GlesTexture>,
) -> Result<GlesTexture, GlesError> {
    let mut texture = renderer.create_buffer(Fourcc::Abgr8888, buffer_size(to))?;
    {
        let mut target = renderer.bind(&mut texture)?;
        let mut frame = renderer.render(&mut target, to, Transform::Normal)?;
        let full = [Rectangle::from_size(to)];
        frame.clear([0.0, 0.0, 0.0, 0.0].into(), &full)?;
        let src = Rectangle::from_size(buffer_size(from).to_f64());
        if let Some(under) = under {
            frame.render_texture_from_to(
                under,
                src,
                Rectangle::from_size(to),
                &full,
                &[],
                Transform::Normal,
                1.0,
                None,
                &[],
            )?;
        }
        frame.render_texture_from_to(
            source,
            src,
            Rectangle::from_size(to),
            &full,
            &[],
            Transform::Normal,
            alpha,
            program,
            uniforms,
        )?;
        // Stesso contesto GL per chi userà la texture: l'ordine dei comandi basta.
        let _sync = frame.finish()?;
    }
    Ok(texture)
}

/// Sfoca la cattura con il σ del materiale e mescola la quota nitida.
fn blur(
    renderer: &mut GlesRenderer,
    programs: &GlassPrograms,
    sharp: &GlesTexture,
    size: Size<i32, Physical>,
    material: MaterialParams,
    scale: f64,
) -> Result<GlesTexture, GlesError> {
    let sigma = material.sigma * scale;
    // Riduzione: con σ grande si sfoca a metà o a un quarto della risoluzione.
    let factor = if sigma > MAX_SIGMA_PX * 2.0 {
        4
    } else if sigma > MAX_SIGMA_PX {
        2
    } else {
        1
    };
    let small = Size::from(((size.w / factor).max(1), (size.h / factor).max(1)));
    let mut current = if factor > 1 {
        pass(renderer, sharp, size, small, None, &[], 1.0, None)?
    } else {
        sharp.clone()
    };
    let sigma_small = (sigma / f64::from(factor)) as f32;
    for step in [(1.0 / small.w as f32, 0.0), (0.0, 1.0 / small.h as f32)] {
        let uniforms = [
            Uniform::new("step", UniformValue::_2f(step.0, step.1)),
            Uniform::new("sigma", UniformValue::_1f(sigma_small)),
        ];
        current = pass(
            renderer,
            &current,
            small,
            small,
            Some(&programs.blur),
            &uniforms,
            1.0,
            None,
        )?;
    }
    // Ritorno alla risoluzione piena, con la quota nitida sotto: w · nitida + (1 − w) · sfocata.
    pass(
        renderer,
        &current,
        small,
        size,
        None,
        &[],
        1.0 - material.sharp,
        Some(sharp),
    )
}

/// Una regione di vetro pronta da disegnare.
#[derive(Debug, Clone)]
pub struct GlassElement {
    id: Id,
    commit: CommitCounter,
    /// Lo sfondo sfocato, grande quanto `bbox`.
    texture: GlesTexture,
    bbox: Rectangle<i32, Physical>,
    region: PhysicalRegion,
    program: GlesTexProgram,
}

impl Element for GlassElement {
    fn id(&self) -> &Id {
        &self.id
    }

    fn current_commit(&self) -> CommitCounter {
        self.commit
    }

    fn src(&self) -> Rectangle<f64, Buffer> {
        let local = Rectangle::new(self.region.rect.loc - self.bbox.loc, self.region.rect.size);
        Rectangle::new(
            Point::from((f64::from(local.loc.x), f64::from(local.loc.y))),
            buffer_size(local.size).to_f64(),
        )
    }

    fn geometry(&self, _scale: Scale<f64>) -> Rectangle<i32, Physical> {
        self.region.rect
    }

    fn kind(&self) -> Kind {
        Kind::Unspecified
    }
}

impl RenderElement<GlesRenderer> for GlassElement {
    fn draw(
        &self,
        frame: &mut GlesFrame<'_, '_>,
        src: Rectangle<f64, Buffer>,
        dst: Rectangle<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
        opaque_regions: &[Rectangle<i32, Physical>],
    ) -> Result<(), GlesError> {
        let local = self.region.rect.loc - self.bbox.loc;
        let size = self.region.rect.size;
        let m = self.region.material.matrix;
        let b = self.region.material.offset;
        let uniforms = [
            Uniform::new(
                "tex_size",
                UniformValue::_2f(self.bbox.size.w as f32, self.bbox.size.h as f32),
            ),
            Uniform::new(
                "region",
                UniformValue::_4f(local.x as f32, local.y as f32, size.w as f32, size.h as f32),
            ),
            Uniform::new("radius", UniformValue::_1f(self.region.radius as f32)),
            Uniform::new("exponent", UniformValue::_1f(CORNER_EXPONENT)),
            Uniform::new("row_r", UniformValue::_3f(m[0][0], m[0][1], m[0][2])),
            Uniform::new("row_g", UniformValue::_3f(m[1][0], m[1][1], m[1][2])),
            Uniform::new("row_b", UniformValue::_3f(m[2][0], m[2][1], m[2][2])),
            Uniform::new("offset", UniformValue::_3f(b[0], b[1], b[2])),
        ];
        frame.render_texture_from_to(
            &self.texture,
            src,
            dst,
            damage,
            opaque_regions,
            Transform::Normal,
            1.0,
            Some(&self.program),
            &uniforms,
        )
    }
}
