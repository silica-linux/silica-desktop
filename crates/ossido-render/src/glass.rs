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
    /// La lente del bordo.
    pub lens: Lens,
}

/// La lente del bordo: per ogni lato, da quale distanza dal bordo (s) il vetro preleva lo
/// sfondo per un pixel a distanza t, in punti, con punti (t, s) a t crescente (al più 8).
/// s > t ingrandisce (preleva più verso l'interno), s < 0 preleva fuori dal bordo. Prima
/// del primo punto vale il primo, oltre l'ultimo s = t.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lens {
    pub side: &'static [(f32, f32)],
    pub top: &'static [(f32, f32)],
    pub bottom: &'static [(f32, f32)],
}

impl Lens {
    /// Quanto lontano fuori dal bordo può prelevare, in punti.
    pub fn outward_reach(&self) -> f32 {
        [self.side, self.top, self.bottom]
            .iter()
            .flat_map(|curve| curve.iter().map(|(_, s)| -s))
            .fold(0.0, f32::max)
    }

    /// Le tre curve come array di `mat4` (colonne: t0, s0, t1, s1), scalate in pixel.
    fn uniform(&self, px: f32) -> UniformValue {
        let pack = |curve: &[(f32, f32)]| {
            // I posti vuoti hanno t enorme: lo shader si ferma lì.
            let mut m = [1.0e6_f32; 16];
            for ([slot_t, slot_s], (t, s)) in m.as_chunks_mut::<2>().0.iter_mut().zip(curve) {
                *slot_t = t * px;
                *slot_s = s * px;
            }
            m
        };
        UniformValue::Matrix4x4 {
            matrices: vec![pack(self.side), pack(self.top), pack(self.bottom)],
            transpose: false,
        }
    }
}

/// Bordo comune ai vetri misurati (Dock, moduli, HUD, notifiche, Spotlight): sul grigio
/// del Dock la riga di 1 pt sopra e sotto vale +60, +60, +63 sul riempimento, la
/// sfumatura interna +13, +8, +5, +3, +2, +1 (≈ 13·e^(−t/2,2)), additiva anche su nero;
/// ai lati 1 pt scuro (`#3c3c40` su `#818289`: nero con alfa 0,535). `dock.md`, `glass.md`.
const EDGE_RIM: [f32; 3] = [60.0 / 255.0, 60.0 / 255.0, 63.0 / 255.0];
const EDGE_GLOW: f32 = 13.0 / 255.0;
const EDGE_GLOW_DECAY_PT: f32 = 2.2;
const EDGE_SIDE_DARK: f32 = 0.535;

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
    lens: DOCK_LENS,
};

/// Lente del Dock: ai lati fino a ~5 pt verso l'interno a 6 pt dal bordo, nulla oltre
/// ~22 pt; sopra ≤ 1 pt, sotto ~3 pt (glass.md, "Vetro · Dock · Rifrazione").
const DOCK_LENS: Lens = Lens {
    side: &[
        (0.0, 0.0),
        (6.0, 11.0),
        (10.0, 14.3),
        (14.0, 16.5),
        (18.0, 18.7),
        (22.0, 22.0),
    ],
    top: &[
        (0.0, 0.0),
        (6.0, 7.0),
        (10.0, 10.9),
        (14.0, 14.5),
        (18.0, 18.1),
        (22.0, 22.0),
    ],
    bottom: &[
        (0.0, 0.0),
        (6.0, 9.0),
        (10.0, 12.6),
        (14.0, 15.5),
        (18.0, 18.4),
        (22.0, 22.0),
    ],
};

/// Moduli del Centro di controllo, chiaro: σ 4,5 pt, w 0,04, matrice libera (rmse 3,33)
/// con b = (30,6; 32,0; 30,3); identica in scuro (glass.md, "Centro di controllo").
pub const REGULAR_LIGHT: MaterialParams = MaterialParams {
    sigma: 4.5,
    sharp: 0.04,
    matrix: [
        [0.946, -0.285, -0.037],
        [-0.126, 0.651, 0.094],
        [-0.140, -0.297, 1.064],
    ],
    offset: [30.6 / 255.0, 32.0 / 255.0, 30.3 / 255.0],
    lens: LIQUID_LENS,
};

/// La lente del Liquid Glass, misurata sui moduli del Centro di controllo: fra ~6 e
/// ~24 pt dal bordo ingrandisce (fino a ~4 pt verso l'interno), nei ~6 pt esterni preleva
/// lo sfondo che sta fuori dal modulo, fino a ~25 pt oltre il bordo, schiacciato. Sopra la
/// fascia esterna è di ~2 pt. Il lato inferiore non è misurato: come quello superiore.
/// Incertezza ±3 pt nella fascia esterna (glass.md, "Rifrazione, molto più forte").
const LIQUID_LENS: Lens = Lens {
    side: &[
        (0.0, -25.0),
        (6.0, -4.0),
        (10.0, 14.2),
        (24.0, 25.5),
        (30.0, 30.0),
    ],
    top: &[
        (0.0, -27.0),
        (2.0, -5.0),
        (6.0, 9.6),
        (19.0, 21.3),
        (28.0, 28.0),
    ],
    bottom: &[
        (0.0, -27.0),
        (2.0, -5.0),
        (6.0, 9.6),
        (19.0, 21.3),
        (28.0, 28.0),
    ],
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
    // Nei menu la sfocatura nasconde la rifrazione: ≤ 1–2 pt ai lati.
    lens: Lens {
        side: &[
            (0.0, 0.0),
            (6.0, 7.5),
            (10.0, 11.3),
            (14.0, 14.75),
            (18.0, 18.2),
            (22.0, 22.0),
        ],
        top: &[],
        bottom: &[],
    },
};

pub const BLUR_SHADER: &str = include_str!("shaders/blur.frag");
pub const BLUR_UNIFORMS: [(&str, UniformType); 2] =
    [("step", UniformType::_2f), ("sigma", UniformType::_1f)];
pub const MATERIAL_SHADER: &str = include_str!("shaders/material.frag");
pub const MATERIAL_UNIFORMS: [(&str, UniformType); 16] = [
    ("tex_size", UniformType::_2f),
    ("shapes", UniformType::Matrix4x4),
    ("radii", UniformType::Matrix4x4),
    ("shape_count", UniformType::_1f),
    ("merge", UniformType::_1f),
    ("exponent", UniformType::_1f),
    ("row_r", UniformType::_3f),
    ("row_g", UniformType::_3f),
    ("row_b", UniformType::_3f),
    ("offset", UniformType::_3f),
    ("scale_px", UniformType::_1f),
    ("lens", UniformType::Matrix4x4),
    ("lens_reach", UniformType::_1f),
    ("rim", UniformType::_3f),
    ("edge", UniformType::_2f),
    ("side_dark", UniformType::_1f),
];

/// Fin dove arriva la lente dentro il vetro (l'ultimo punto delle curve), in punti: la
/// normale si calcola su angoli almeno così ampi, perché sulla diagonale di un angolo
/// retto non salti.
fn lens_extent(lens: &Lens) -> f32 {
    [lens.side, lens.top, lens.bottom]
        .iter()
        .filter_map(|curve| curve.last().map(|(t, _)| *t))
        .fold(0.0, f32::max)
}

/// Livelli di qualità del vetro: pieno (sfoca tutto quello che sta dietro), smerigliato
/// statico (solo lo sfondo del desktop: si ricalcola di rado), opaco (il materiale sopra il
/// colore del desktop, senza sfocatura). Per le GPU deboli (piano, "Rischi").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlassQuality {
    Full,
    Frosted,
    Opaque,
}

impl GlassQuality {
    pub fn name(self) -> &'static str {
        match self {
            GlassQuality::Full => "full",
            GlassQuality::Frosted => "frosted",
            GlassQuality::Opaque => "opaque",
        }
    }
}

/// Spazio fra due regioni dello stesso materiale sotto il quale si uniscono in una sola
/// forma, come gocce (unione morbida delle distanze, k = 2 · MERGE_PT: due bordi a distanza
/// g si toccano quando g < k / 2). Limite misurato: i pulsanti circolari di Spotlight stanno
/// a 10 pt dal pannello e restano separati (spotlight.md), quindi a riposo la soglia è sotto
/// i 10 pt. Il valore esatto non è misurato (plan/execution-plan.md, "Da misurare sul Mac
/// per il Liquid Glass").
pub const MERGE_PT: f32 = 8.0;
/// Regioni che lo shader fonde insieme al più: oltre, gruppi separati.
const MAX_SHAPES: usize = 8;

/// Oltre questo σ (pixel) si sfoca a metà o a un quarto della risoluzione: il nucleo
/// dello shader arriva a 48 pixel.
const MAX_SIGMA_PX: f64 = 12.0;

/// Una regione di vetro nell'output, in pixel fisici.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhysicalRegion {
    pub rect: Rectangle<i32, Physical>,
    pub radius: f64,
    pub material: MaterialParams,
    /// Pixel fisici per punto.
    pub scale: f64,
}

/// Lo stato degli elementi dietro il vetro: se non cambia, la texture si riusa.
type Signature = Vec<(Id, CommitCounter, Rectangle<i32, Physical>)>;

/// Il vetro di una superficie fra un frame e l'altro (nei dati della superficie).
#[derive(Default)]
pub struct GlassCache {
    state: RefCell<Option<Cached>>,
}

struct Cached {
    quality: GlassQuality,
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
    /// dal primo piano allo sfondo) e la sua parte fissa (`behind_static`: sfondo del
    /// desktop, per lo smerigliato). Errore se il renderer non riesce a disegnare fuori
    /// schermo: il chiamante salta il vetro per questo frame.
    #[allow(clippy::too_many_arguments)]
    pub fn elements<E: RenderElement<GlesRenderer>>(
        &self,
        renderer: &mut GlesRenderer,
        programs: &GlassPrograms,
        regions: &[PhysicalRegion],
        behind: &[E],
        behind_static: &[E],
        quality: GlassQuality,
        output: Rectangle<i32, Physical>,
        scale: f64,
        clear_color: [f32; 4],
    ) -> Result<Vec<GlassElement>, GlesError> {
        let behind = match quality {
            GlassQuality::Full => behind,
            GlassQuality::Frosted => behind_static,
            GlassQuality::Opaque => behind.get(..0).unwrap_or_default(),
        };
        let margin = regions
            .iter()
            .map(|region| {
                let reach = f64::from(region.material.lens.outward_reach() + MERGE_PT);
                ((region.material.sigma * 3.0 + reach) * scale).ceil() as i32 + 2
            })
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
            cached.quality == quality
                && cached.signature == signature
                && cached.regions == regions
                && cached.bbox == bbox
        });
        if !reuse {
            let sharp = render_to_texture(renderer, behind, bbox, scale, clear_color)?;
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
            let count = groups(regions).len();
            let (ids, mut commit) = match state.take() {
                Some(cached) if cached.ids.len() == count => (cached.ids, cached.commit),
                Some(cached) => ((0..count).map(|_| Id::new()).collect(), cached.commit),
                None => (
                    (0..count).map(|_| Id::new()).collect(),
                    CommitCounter::default(),
                ),
            };
            commit.increment();
            *state = Some(Cached {
                quality,
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
        Ok(groups(&cached.regions)
            .into_iter()
            .zip(&cached.ids)
            .filter_map(|(shapes, id)| {
                let material = shapes.first()?.material;
                let texture = cached
                    .textures
                    .iter()
                    .find(|(m, _)| *m == material)?
                    .1
                    .clone();
                let scale = shapes.first()?.scale;
                // L'unione morbida esce dalle regioni al più di k / 4 = MERGE_PT / 2.
                let bulge = (f64::from(MERGE_PT) * scale * 0.5).ceil() as i32 + 1;
                let area = shapes
                    .iter()
                    .map(|shape| shape.rect)
                    .reduce(|a, b| a.merge(b))?;
                let area = Rectangle::new(
                    area.loc - Point::from((bulge, bulge)),
                    area.size + Size::from((bulge.saturating_mul(2), bulge.saturating_mul(2))),
                )
                .intersection(cached.bbox)?;
                Some(GlassElement {
                    id: id.clone(),
                    commit: cached.commit,
                    texture,
                    bbox: cached.bbox,
                    area,
                    shapes,
                    program: programs.material.clone(),
                })
            })
            .collect())
    }
}

/// Le regioni raggruppate per materiale (nell'ordine in cui compaiono), al più
/// `MAX_SHAPES` per gruppo: le regioni di un gruppo si fondono fra loro.
fn groups(regions: &[PhysicalRegion]) -> Vec<Vec<PhysicalRegion>> {
    let mut groups: Vec<Vec<PhysicalRegion>> = Vec::new();
    for region in regions {
        match groups.iter_mut().find(|group| {
            group.len() < MAX_SHAPES
                && group
                    .first()
                    .is_some_and(|first| first.material == region.material)
        }) {
            Some(group) => group.push(*region),
            None => groups.push(vec![*region]),
        }
    }
    groups
}

fn buffer_size(size: Size<i32, Physical>) -> Size<i32, Buffer> {
    size.to_logical(1).to_buffer(1, Transform::Normal)
}

/// Disegna `elements` (dal primo piano allo sfondo) sopra `clear_color` in una texture
/// nuova che copre `bbox` dell'output: lo sfondo del vetro, l'istantanea di una finestra
/// che si chiude.
pub fn render_to_texture<E: RenderElement<GlesRenderer>>(
    renderer: &mut GlesRenderer,
    elements: &[E],
    bbox: Rectangle<i32, Physical>,
    scale: f64,
    clear_color: [f32; 4],
) -> Result<GlesTexture, GlesError> {
    let mut texture = renderer.create_buffer(Fourcc::Abgr8888, buffer_size(bbox.size))?;
    {
        let mut target = renderer.bind(&mut texture)?;
        let mut frame = renderer.render(&mut target, bbox.size, Transform::Normal)?;
        frame.clear(clear_color.into(), &[Rectangle::from_size(bbox.size)])?;
        for element in elements.iter().rev() {
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
    /// Dove si disegna: le regioni del gruppo più lo spazio della fusione.
    area: Rectangle<i32, Physical>,
    /// Le regioni del gruppo (stesso materiale, al più `MAX_SHAPES`).
    shapes: Vec<PhysicalRegion>,
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
        let local = Rectangle::new(self.area.loc - self.bbox.loc, self.area.size);
        Rectangle::new(
            Point::from((f64::from(local.loc.x), f64::from(local.loc.y))),
            buffer_size(local.size).to_f64(),
        )
    }

    fn geometry(&self, _scale: Scale<f64>) -> Rectangle<i32, Physical> {
        self.area
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
        let Some(first) = self.shapes.first() else {
            return Ok(());
        };
        let m = first.material.matrix;
        let b = first.material.offset;
        let lens = first.material.lens;
        let px = first.scale as f32;
        // Le regioni rispetto alla texture: fino a 8 rettangoli in due mat4 (una colonna
        // per rettangolo: x, y, larghezza, altezza) e i raggi in una mat4.
        let mut rects = [[0.0_f32; 16]; 2];
        let mut radii = [0.0_f32; 16];
        for (i, shape) in self.shapes.iter().take(MAX_SHAPES).enumerate() {
            let local = shape.rect.loc - self.bbox.loc;
            let values = [
                local.x as f32,
                local.y as f32,
                shape.rect.size.w as f32,
                shape.rect.size.h as f32,
            ];
            if let Some(slot) = rects
                .get_mut(i / 4)
                .and_then(|matrix| matrix.get_mut((i % 4) * 4..(i % 4) * 4 + 4))
            {
                slot.copy_from_slice(&values);
            }
            if let Some(slot) = radii.get_mut(i) {
                *slot = shape.radius as f32;
            }
        }
        let uniforms = [
            Uniform::new(
                "tex_size",
                UniformValue::_2f(self.bbox.size.w as f32, self.bbox.size.h as f32),
            ),
            Uniform::new(
                "shapes",
                UniformValue::Matrix4x4 {
                    matrices: rects.to_vec(),
                    transpose: false,
                },
            ),
            Uniform::new(
                "radii",
                UniformValue::Matrix4x4 {
                    matrices: vec![radii],
                    transpose: false,
                },
            ),
            Uniform::new(
                "shape_count",
                UniformValue::_1f(self.shapes.len().min(MAX_SHAPES) as f32),
            ),
            Uniform::new("merge", UniformValue::_1f(2.0 * MERGE_PT * px)),
            Uniform::new("exponent", UniformValue::_1f(2.0)),
            Uniform::new("row_r", UniformValue::_3f(m[0][0], m[0][1], m[0][2])),
            Uniform::new("row_g", UniformValue::_3f(m[1][0], m[1][1], m[1][2])),
            Uniform::new("row_b", UniformValue::_3f(m[2][0], m[2][1], m[2][2])),
            Uniform::new("offset", UniformValue::_3f(b[0], b[1], b[2])),
            Uniform::new("scale_px", UniformValue::_1f(px)),
            Uniform::new("lens", lens.uniform(px)),
            Uniform::new("lens_reach", UniformValue::_1f(lens_extent(&lens) * px)),
            Uniform::new(
                "rim",
                UniformValue::_3f(EDGE_RIM[0], EDGE_RIM[1], EDGE_RIM[2]),
            ),
            Uniform::new(
                "edge",
                UniformValue::_2f(EDGE_GLOW, EDGE_GLOW_DECAY_PT * px),
            ),
            Uniform::new("side_dark", UniformValue::_1f(EDGE_SIDE_DARK)),
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
