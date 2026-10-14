//! La lista degli elementi di un output con GLES: layer, finestre ed effetti di
//! `ossido-render`, dal primo piano allo sfondo. Con pixman (headless senza EGL) si usa
//! ancora `space_render_elements`, senza effetti.

use std::cell::Cell;
use std::time::Instant;

use ossido_render::animation;
use ossido_render::corners::{RoundedSurface, WindowSurface};
use ossido_render::glass::{
    CLEAR_LIGHT, GlassCache, GlassElement, GlassQuality, NOTIFICATION_LIGHT, PhysicalRegion,
    REGULAR_LIGHT, THICK_LIGHT, render_to_texture,
};
use ossido_render::shadow::{ACTIVE_LIGHT, INACTIVE_LIGHT, WindowShadow};
use ossido_render::title::Title;
use ossido_render::titlebar::{Titlebar, TitlebarState};
use smithay::backend::renderer::element::solid::SolidColorRenderElement;
use smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement;
use smithay::backend::renderer::element::texture::TextureRenderElement;
use smithay::backend::renderer::element::utils::RescaleRenderElement;
use smithay::backend::renderer::element::{AsRenderElements, Id, Kind};
use smithay::backend::renderer::gles::element::PixelShaderElement;
use smithay::backend::renderer::gles::{GlesRenderer, GlesTexture};
use smithay::backend::renderer::utils::CommitCounter;
use smithay::backend::renderer::{Renderer, Texture};
use smithay::desktop::{LayerSurface, Space, Window, layer_map_for_output};
use smithay::output::Output;
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::render_elements;
use smithay::utils::{Logical, Physical, Point, Rectangle, Size, Transform};
use smithay::wayland::compositor::with_states;
use smithay::wayland::shell::wlr_layer::Layer;
use tracing::warn;

use crate::decorations;
use crate::glass::{Material, Shape, current_regions};

render_elements! {
    pub OutputElement<=GlesRenderer>;
    Surface=WaylandSurfaceRenderElement<GlesRenderer>,
    Rounded=RoundedSurface,
    /// Elementi a pixel shader della finestra: barra del titolo e ombra.
    Shader=RescaleRenderElement<PixelShaderElement>,
    Glass=GlassElement,
    /// Il gradiente sotto la barra dei menu, sopra lo sfondo del desktop.
    Tone=ossido_render::backdrop::ToneElement,
    /// Texture: titoli delle barre, istantanee delle finestre che si chiudono.
    Texture=RescaleRenderElement<TextureRenderElement<GlesTexture>>,
    /// Una finestra che si riduce nel Dock o ne esce, deformata a genio.
    Genie=ossido_render::genie::GenieElement,
    /// La fascia nera fra due spazi che scorrono.
    Solid=SolidColorRenderElement,
}

/// Gli strati che appartengono allo spazio e scorrono con lui: lo sfondo e la barra dei
/// menu (nel riferimento il Dock e i pannelli della shell restano fermi).
fn moves_with_space(layer: &LayerSurface, level: Layer) -> bool {
    matches!(level, Layer::Background | Layer::Bottom) || layer.namespace() == "menubar"
}

/// Quello che serve al frame oltre allo spazio.
pub struct RenderContext<'a> {
    /// I pulsanti della barra di una finestra: simboli e pulsante premuto.
    pub buttons: Option<ButtonsView>,
    /// ⌥ tenuto: il verde mostra "+".
    pub alt: bool,
    pub clear_color: [f32; 4],
    /// Moltiplicatore delle durate delle animazioni (0: nessuna animazione).
    pub animation_speed: f64,
    pub closing: &'a [Closing],
    /// Finestre che si riducono nel Dock o ne escono.
    pub genies: &'a [Genie],
    pub now: Instant,
    pub glass_quality: GlassQuality,
    /// Cambio di spazio in corso: sfondo e barra dei menu scorrono con le finestre.
    pub space_shift: Option<crate::workspaces::SpaceShift>,
    /// Panorama aperto o in chiusura: finestre disposte, barra degli spazi.
    pub panorama: Option<crate::panorama::PanoramaView>,
    /// Numero di spazi e spazio attivo, per la barra di Panorama.
    pub spaces: (usize, usize),
}

/// I pulsanti della barra di una finestra nel frame.
#[derive(Clone)]
pub struct ButtonsView {
    pub window: Window,
    pub symbols: f32,
    pub pressed: Option<u8>,
}

/// Come disegnare i pulsanti di una finestra.
#[derive(Clone, Copy, Default)]
struct ButtonsLook {
    symbols: f32,
    pressed: Option<u8>,
    alt: bool,
}

/// Riduzione e schermo intero non hanno senso: dialogo (ha un genitore) o finestra di
/// dimensione fissa (minima uguale alla massima).
fn buttons_disabled(window: &Window) -> bool {
    let Some(toplevel) = window.toplevel() else {
        return false;
    };
    if toplevel.parent().is_some() {
        return true;
    }
    with_states(toplevel.wl_surface(), |states| {
        let mut cached = states
            .cached_state
            .get::<smithay::wayland::shell::xdg::SurfaceCachedState>();
        let current = cached.current();
        !current.min_size.is_empty() && current.min_size == current.max_size
    })
}

/// Una finestra chiusa che svanisce: l'istantanea dell'ultimo aspetto.
pub struct Closing {
    pub id: Id,
    pub texture: GlesTexture,
    /// Dove sta l'istantanea nell'output, in pixel fisici.
    pub bbox: Rectangle<i32, Physical>,
    /// Centro della cornice, attorno a cui la finestra si riduce.
    pub center: Point<i32, Physical>,
    pub start: Instant,
}

impl Closing {
    pub fn finished(&self, now: Instant, speed: f64) -> bool {
        animation::progress(now - self.start, animation::CLOSE, speed) >= 1.0
    }
}

/// Una finestra che si riduce nel Dock o ne esce: l'istantanea che si deforma a genio.
pub struct Genie {
    pub id: Id,
    /// Cambia a ogni frame: la forma cambia anche se l'istantanea è la stessa.
    pub commit: std::cell::Cell<smithay::backend::renderer::utils::CommitCounter>,
    pub texture: GlesTexture,
    /// L'istantanea nell'output, in pixel fisici.
    pub snapshot: Rectangle<i32, Physical>,
    /// La cornice della finestra (senza ombra) e la miniatura, in pixel fisici.
    pub frame: Rectangle<f64, Physical>,
    pub target: Rectangle<f64, Physical>,
    pub start: Instant,
    /// Il ripristino: la finestra è già mappata ma resta nascosta finché il genio non finisce.
    pub restoring: Option<Window>,
}

impl Genie {
    fn duration(&self) -> std::time::Duration {
        if self.restoring.is_some() {
            animation::RESTORE
        } else {
            animation::MINIMIZE
        }
    }

    pub fn finished(&self, now: Instant, speed: f64) -> bool {
        let gone = self
            .restoring
            .as_ref()
            .is_some_and(|w| !smithay::utils::IsAlive::alive(w));
        gone || animation::progress(now - self.start, self.duration(), speed) >= 1.0
    }

    /// La forma all'istante `now`.
    pub fn shape(&self, now: Instant, speed: f64) -> animation::GenieShape {
        let duration = self.duration();
        let progress =
            animation::progress(now.saturating_duration_since(self.start), duration, speed);
        let ms = progress * duration.as_secs_f64() * 1000.0;
        if self.restoring.is_some() {
            animation::genie_restore(ms)
        } else {
            animation::genie_minimize(ms)
        }
    }

    /// La finestra in ripristino che il genio nasconde ancora.
    pub fn hides(&self, window: &Window) -> bool {
        self.restoring.as_ref() == Some(window)
    }
}

/// Lo schermo intero che entra o esce: il rettangolo della cornice va da `from` a `to`, nei
/// dati della finestra.
#[derive(Default)]
pub struct FullscreenMorph(pub std::cell::Cell<Option<Morph>>);

#[derive(Clone, Copy)]
pub struct Morph {
    pub from: Rectangle<f64, Logical>,
    pub to: Rectangle<f64, Logical>,
    pub start: Instant,
    pub entering: bool,
}

impl Morph {
    fn duration(&self) -> std::time::Duration {
        if self.entering {
            animation::FULLSCREEN_ENTER
        } else {
            animation::FULLSCREEN_EXIT
        }
    }

    /// Il rettangolo all'istante `now`; None a animazione finita.
    pub fn rect(&self, now: Instant, speed: f64) -> Option<Rectangle<f64, Logical>> {
        let duration = self.duration();
        let progress =
            animation::progress(now.saturating_duration_since(self.start), duration, speed);
        if progress >= 1.0 {
            return None;
        }
        let ms = progress * duration.as_secs_f64() * 1000.0;
        let f = if self.entering {
            animation::fullscreen_enter(ms)
        } else {
            animation::fullscreen_exit(ms)
        };
        let lerp = |a: f64, b: f64| a + (b - a) * f;
        Some(Rectangle::new(
            (
                lerp(self.from.loc.x, self.to.loc.x),
                lerp(self.from.loc.y, self.to.loc.y),
            )
                .into(),
            (
                lerp(self.from.size.w, self.to.size.w),
                lerp(self.from.size.h, self.to.size.h),
            )
                .into(),
        ))
    }
}

/// Il rettangolo dello schermo intero in corso per una finestra (e toglie quello finito).
fn morph_rect(window: &Window, context: &RenderContext<'_>) -> Option<Rectangle<f64, Logical>> {
    let morph = window.user_data().get::<FullscreenMorph>()?;
    let current = morph.0.get()?;
    let rect = current.rect(context.now, context.animation_speed);
    if rect.is_none() {
        morph.0.set(None);
    }
    rect
}

/// Id stabile della fascia nera, nei dati dell'output.
struct SpaceGapId(Id);

impl Default for SpaceGapId {
    fn default() -> Self {
        Self(Id::new())
    }
}

/// La sfumatura in alto di App Exposé, nei dati dell'output.
#[derive(Default)]
struct AppShade(ossido_render::sprite::Sprite);

/// L'etichetta di App Exposé di una finestra, nei suoi dati.
#[derive(Default)]
struct AppLabel(ossido_render::sprite::Sprite);

/// I titoli delle finestre di App Exposé, sotto le cornici disposte.
fn app_labels(
    renderer: &mut GlesRenderer,
    elements: &mut Vec<OutputElement>,
    panorama: &crate::panorama::PanoramaView,
    scale: f64,
) {
    use ossido_render::sprite::{Weight, text_width};
    let size = crate::panorama::APP_LABEL_SIZE;
    for placement in &panorama.placements {
        let title = window_title(&placement.window);
        if title.is_empty() {
            continue;
        }
        let rect = crate::panorama::interpolate(placement, panorama.progress);
        let width = f64::from(text_width(&title, size, Weight::Regular)).ceil() + 4.0;
        let height = f64::from(size) + 8.0;
        // Maiuscole (~9 pt) che cominciano 14 pt sotto la cornice.
        let baseline = f64::from(size) * 0.75 + 2.0;
        let location = Point::from((
            rect.loc.x + (rect.size.w - width) / 2.0,
            rect.loc.y + rect.size.h + crate::panorama::APP_LABEL_GAP + 9.0 - baseline,
        ));
        placement
            .window
            .user_data()
            .insert_if_missing(AppLabel::default);
        let Some(label) = placement.window.user_data().get::<AppLabel>() else {
            continue;
        };
        let element = label.0.element(
            renderer,
            &title,
            Size::from((width, height)),
            location,
            scale,
            panorama.progress as f32,
            |canvas| {
                // Bianco con un'ombra scura sotto.
                canvas.text(
                    [2.0, baseline as f32 + 1.0],
                    &title,
                    size,
                    Weight::Regular,
                    [0, 0, 0, 90],
                );
                canvas.text(
                    [2.0, baseline as f32],
                    &title,
                    size,
                    Weight::Regular,
                    [0xfe, 0xfe, 0xfe, 0xff],
                );
            },
        );
        if let Some(element) = element {
            elements.push(OutputElement::Texture(RescaleRenderElement::from_element(
                element,
                Point::default(),
                1.0,
            )));
        }
    }
}

/// Quando la finestra ha mostrato il primo buffer: l'apertura parte da lì.
#[derive(Default)]
struct Opened(Cell<Option<Instant>>);

/// Dove va il vetro di una superficie: subito dietro le sue superfici nella lista.
struct GlassAnchor {
    index: usize,
    surface: WlSurface,
    /// Origine della superficie nell'output, in pixel fisici.
    origin: Point<i32, Physical>,
    /// Opacità dello strato (la barra dei menu svanisce con Panorama).
    opacity: f32,
}

/// La finestra ha lo stato attivo (focus), anche se il configure non è ancora partito.
fn is_active(window: &Window) -> bool {
    window.toplevel().is_some_and(|toplevel| {
        toplevel.with_pending_state(|state| state.states.contains(xdg_toplevel::State::Activated))
    })
}

/// Il titolo della finestra (`xdg_toplevel.set_title`), vuoto se non c'è.
fn window_title(window: &Window) -> String {
    window
        .toplevel()
        .and_then(|toplevel| {
            with_states(toplevel.wl_surface(), |states| {
                states
                    .data_map
                    .get::<smithay::wayland::shell::xdg::XdgToplevelSurfaceData>()
                    .and_then(|data| data.lock().ok())
                    .and_then(|data| data.title.clone())
            })
        })
        .unwrap_or_default()
}

/// La scala dell'apertura in questo momento (1 a riposo).
fn open_scale(window: &Window, context: &RenderContext<'_>) -> f64 {
    window.user_data().insert_if_missing(Opened::default);
    let Some(opened) = window.user_data().get::<Opened>() else {
        return 1.0;
    };
    let start = match opened.0.get() {
        Some(start) => start,
        // Senza contenuto non c'è ancora niente da mostrare.
        None if window.geometry().size.is_empty() => return 1.0,
        None => {
            opened.0.set(Some(context.now));
            context.now
        }
    };
    let progress = animation::progress(
        context.now.saturating_duration_since(start),
        animation::OPEN,
        context.animation_speed,
    );
    animation::open_scale(progress)
}

/// Un rettangolo scalato attorno a `center`.
fn scale_rect(
    rect: Rectangle<f64, Physical>,
    center: Point<f64, Physical>,
    factor: f64,
) -> Rectangle<f64, Physical> {
    let loc = center + (rect.loc - center).upscale(factor);
    Rectangle::new(loc, rect.size.upscale(factor))
}

/// Dove sta una finestra dello Space: posizione e geometria.
fn placed(
    space: &Space<Window>,
    window: &Window,
) -> Option<(Point<i32, Logical>, Rectangle<i32, Logical>)> {
    Some((
        space.element_location(window)?,
        space.element_geometry(window)?,
    ))
}

/// La cornice di una finestra in punti, dalla sua geometria.
fn frame_rect(window: &Window, geometry: Rectangle<i32, Logical>) -> Rectangle<f64, Logical> {
    decorations::frame_of(window, geometry).to_f64()
}

/// Spostamento e scala che portano la cornice `from` su `to` (scala attorno al centro).
fn fit(from: Rectangle<f64, Logical>, to: Rectangle<f64, Logical>) -> (Point<i32, Logical>, f64) {
    let center = |r: Rectangle<f64, Logical>| r.loc + r.size.downscale(2.0).to_point();
    let offset = (center(to) - center(from)).to_i32_round();
    let factor = if from.size.w > 0.0 {
        to.size.w / from.size.w
    } else {
        1.0
    };
    (offset, factor)
}

/// Gli elementi di una finestra, dal primo piano allo sfondo (superfici, barra, ombra),
/// spostati di `offset` e scalati di `factor` attorno al centro della cornice.
#[allow(clippy::too_many_arguments)]
fn window_elements(
    renderer: &mut GlesRenderer,
    shaders: &ossido_render::Shaders,
    (location, geometry): (Point<i32, Logical>, Rectangle<i32, Logical>),
    window: &Window,
    output_origin: Point<i32, Logical>,
    output_size: Size<i32, Physical>,
    scale: f64,
    offset: Point<i32, Logical>,
    factor: f64,
    buttons: ButtonsLook,
) -> Vec<OutputElement> {
    let render_location = (location - window.geometry().loc - output_origin + offset)
        .to_physical_precise_round(scale);
    let surfaces = window.render_elements::<WaylandSurfaceRenderElement<GlesRenderer>>(
        renderer,
        render_location,
        scale.into(),
        1.0,
    );
    // A schermo intero niente angoli, barra, ombra né animazione (come nelle riprese).
    if crate::windows::is_fullscreen(window) {
        return surfaces.into_iter().map(OutputElement::Surface).collect();
    }
    let mut frame = decorations::frame_of(window, geometry);
    frame.loc -= output_origin;
    frame.loc += offset;
    let frame_physical = frame.to_f64().to_physical(scale);
    let center_f = frame_physical.loc + frame_physical.size.downscale(2.0).to_point();
    let center = center_f.to_i32_round();
    let mut elements: Vec<OutputElement> = surfaces
        .into_iter()
        .map(|surface| {
            OutputElement::Rounded(RoundedSurface::new(
                WindowSurface::from_element(surface, center, factor),
                scale_rect(frame_physical, center_f, factor),
                scale * factor,
                output_size,
                shaders.corners.clone(),
            ))
        })
        .collect();
    let active = is_active(window);
    if decorations::is_server_side(window)
        && let Some(program) = &shaders.titlebar
    {
        let full_size = decorations::full_size_toolbar(window);
        window.user_data().insert_if_missing(Title::default);
        // A tutta finestra il titolo lo scrive l'app nella sua barra.
        if full_size.is_none()
            && let Some(title) = window.user_data().get::<Title>()
            && let Some(text) = title.element(renderer, &window_title(window), frame, active, scale)
        {
            elements.push(OutputElement::Texture(RescaleRenderElement::from_element(
                text, center, factor,
            )));
        }
        window.user_data().insert_if_missing(Titlebar::default);
        if let Some(titlebar) = window.user_data().get::<Titlebar>() {
            let state = TitlebarState {
                active,
                symbols: buttons.symbols,
                pressed: buttons.pressed,
                alt: buttons.alt,
                disabled: buttons_disabled(window),
                scale,
                bar: full_size.is_none(),
                button_y: decorations::button_center_y(window) as f32,
            };
            let element = OutputElement::Shader(RescaleRenderElement::from_element(
                titlebar.element(program, frame, state),
                center,
                factor,
            ));
            // A tutta finestra i pulsanti stanno sopra il contenuto dell'app (la lista va dal
            // primo piano in giù).
            if full_size.is_some() {
                elements.insert(0, element);
            } else {
                elements.push(element);
            }
        }
    }
    if let Some(program) = &shaders.shadow {
        let style = if active { ACTIVE_LIGHT } else { INACTIVE_LIGHT };
        window.user_data().insert_if_missing(WindowShadow::default);
        if let Some(shadow) = window.user_data().get::<WindowShadow>() {
            elements.push(OutputElement::Shader(RescaleRenderElement::from_element(
                shadow.element(program, frame, style),
                center,
                factor,
            )));
        }
    }
    elements
}

/// Il genio di una finestra ancora nello spazio, verso (o da) `target` in punti dell'output.
pub fn snapshot_genie(
    renderer: &mut GlesRenderer,
    space: &Space<Window>,
    output: &Output,
    window: &Window,
    target: Rectangle<f64, Logical>,
    restoring: bool,
) -> Option<Genie> {
    let snapshot = snapshot_window(renderer, space, output, window)?;
    let scale = output.current_scale().fractional_scale();
    let origin = space.output_geometry(output)?.loc.to_f64();
    let mut frame = frame_rect(window, space.element_geometry(window)?);
    frame.loc -= origin;
    let mut target = target;
    target.loc -= origin;
    Some(Genie {
        id: Id::new(),
        commit: std::cell::Cell::default(),
        texture: snapshot.texture,
        snapshot: snapshot.bbox,
        frame: frame.to_physical(scale),
        target: target.to_physical(scale),
        start: Instant::now(),
        restoring: restoring.then(|| window.clone()),
    })
}

/// L'istantanea di una finestra (superfici, barra e ombra) per l'animazione di chiusura.
pub fn snapshot_window(
    renderer: &mut GlesRenderer,
    space: &Space<Window>,
    output: &Output,
    window: &Window,
) -> Option<Closing> {
    let shaders = ossido_render::shaders(renderer);
    let scale = output.current_scale().fractional_scale();
    let output_size = output.current_mode()?.size;
    let output_origin = space.output_geometry(output)?.loc;
    let elements = window_elements(
        renderer,
        &shaders,
        placed(space, window)?,
        window,
        output_origin,
        output_size,
        scale,
        Point::default(),
        1.0,
        ButtonsLook::default(),
    );
    let bbox = elements
        .iter()
        .map(|element| {
            smithay::backend::renderer::element::Element::geometry(element, scale.into())
        })
        .reduce(|a, b| a.merge(b))?
        .intersection(Rectangle::from_size(output_size))?;
    let mut frame = decorations::frame_of(window, space.element_geometry(window)?);
    frame.loc -= output_origin;
    let frame = frame.to_f64().to_physical(scale);
    let center = (frame.loc + frame.size.downscale(2.0).to_point()).to_i32_round();
    match render_to_texture(renderer, &elements, bbox, scale, [0.0; 4]) {
        Ok(texture) => Some(Closing {
            id: Id::new(),
            texture,
            bbox,
            center,
            start: Instant::now(),
        }),
        Err(err) => {
            warn!("istantanea della finestra chiusa non riuscita: {err:?}");
            None
        }
    }
}

/// L'indice di un vetro dopo che un elemento è stato inserito in `at`, davanti allo sfondo:
/// si sposta solo il vetro degli strati di sfondo (indice oltre `at`). Quello di uno strato
/// che finisce proprio dove comincia lo sfondo (nessuna finestra in mezzo, come la barra dei
/// menu su un desktop vuoto) resta davanti all'elemento inserito.
fn after_insert(index: usize, at: usize) -> usize {
    if index > at { index + 1 } else { index }
}

/// Gli elementi dell'output, dal primo piano allo sfondo.
pub fn output_elements(
    renderer: &mut GlesRenderer,
    space: &Space<Window>,
    output: &Output,
    context: &RenderContext<'_>,
) -> Vec<OutputElement> {
    let shaders = ossido_render::shaders(renderer);
    let scale = output.current_scale().fractional_scale();
    let output_size = output
        .current_mode()
        .map(|mode| mode.size)
        .unwrap_or_default();
    let output_origin = space
        .output_geometry(output)
        .map(|geo| geo.loc)
        .unwrap_or_default();
    let mut elements = Vec::new();
    let mut anchors: Vec<GlassAnchor> = Vec::new();

    let layer_map = layer_map_for_output(output);
    let push_layers = |elements: &mut Vec<OutputElement>,
                       anchors: &mut Vec<GlassAnchor>,
                       renderer: &mut GlesRenderer,
                       level| {
        for layer in layer_map.layers_on(level).rev() {
            let Some(geometry) = layer_map.layer_geometry(layer) else {
                continue;
            };
            // Nel cambio di spazio gli strati dello spazio si disegnano due volte: con la
            // scrivania che esce e con quella che entra.
            let offsets: Vec<f64> = match context.space_shift {
                Some(shift) if moves_with_space(layer, level) => {
                    vec![shift.outgoing, shift.incoming]
                }
                _ => vec![0.0],
            };
            let alpha = match &context.panorama {
                Some(panorama)
                    if layer.namespace() == "menubar"
                        && panorama.mode != crate::panorama::Mode::Desktop =>
                {
                    (1.0 - panorama.progress).clamp(0.0, 1.0) as f32
                }
                _ => 1.0,
            };
            for offset in offsets {
                let location = (geometry.loc.to_f64() + Point::from((offset, 0.0)))
                    .to_physical_precise_round(scale);
                elements.extend(
                    AsRenderElements::<GlesRenderer>::render_elements::<
                        WaylandSurfaceRenderElement<GlesRenderer>,
                    >(
                        layer as &LayerSurface,
                        renderer,
                        location,
                        scale.into(),
                        alpha,
                    )
                    .into_iter()
                    .map(OutputElement::Surface),
                );
                anchors.push(GlassAnchor {
                    index: elements.len(),
                    surface: layer.wl_surface().clone(),
                    origin: location,
                    opacity: alpha,
                });
            }
        }
    };
    push_layers(&mut elements, &mut anchors, renderer, Layer::Overlay);
    // Panorama: la finestra trascinata, le miniature degli spazi e la barra degli spazi, che
    // scende sopra la barra dei menu che svanisce.
    if let Some(panorama) = &context.panorama {
        let output_logical = output_size.to_f64().to_logical(scale);
        if let Some((window, rect)) = &panorama.drag
            && let Some(at) = placed(space, window)
        {
            let (offset, factor) = fit(frame_rect(window, at.1), *rect);
            elements.extend(window_elements(
                renderer,
                &shaders,
                at,
                window,
                output_origin,
                output_size,
                scale,
                offset,
                factor,
                ButtonsLook::default(),
            ));
        }
        for thumbnail in &panorama.thumbnails {
            let s = thumbnail.rect.size.w / output_logical.w.max(1.0);
            for (window, location) in thumbnail.windows.iter().rev() {
                let geometry = Rectangle::new(*location, window.geometry().size);
                let frame = frame_rect(window, geometry);
                let target = Rectangle::new(
                    thumbnail.rect.loc + (frame.loc - output_origin.to_f64()).upscale(s),
                    frame.size.upscale(s),
                );
                let (offset, factor) = fit(frame, target);
                elements.extend(window_elements(
                    renderer,
                    &shaders,
                    (*location, geometry),
                    window,
                    output_origin,
                    output_size,
                    scale,
                    offset,
                    factor,
                    ButtonsLook::default(),
                ));
            }
            // Lo sfondo del desktop in piccolo.
            for layer in layer_map.layers_on(Layer::Background).rev() {
                let Some(geometry) = layer_map.layer_geometry(layer) else {
                    continue;
                };
                let location = (thumbnail.rect.loc + geometry.loc.to_f64().upscale(s))
                    .to_physical_precise_round(scale);
                elements.extend(
                    AsRenderElements::<GlesRenderer>::render_elements::<
                        WaylandSurfaceRenderElement<GlesRenderer>,
                    >(
                        layer as &LayerSurface,
                        renderer,
                        location,
                        (scale * s).into(),
                        1.0,
                    )
                    .into_iter()
                    .map(OutputElement::Surface),
                );
            }
        }
        if panorama.mode == crate::panorama::Mode::App {
            app_labels(renderer, &mut elements, panorama, scale);
        }
        let sprite = output
            .user_data()
            .get_or_insert(ossido_render::sprite::Sprite::default);
        let to_u8 = |c: f32| (c.clamp(0.0, 1.0) * 255.0).round() as u8;
        let look = crate::panorama::BarLook {
            width: output_logical.w,
            output: output_logical,
            count: context.spaces.0,
            active: context.spaces.1,
            expansion: panorama.expansion,
            backdrop: context.clear_color.map(to_u8),
        };
        if panorama.mode == crate::panorama::Mode::Panorama
            && let Some(bar) =
                crate::panorama::bar_element(renderer, sprite, &look, panorama.progress, scale)
        {
            elements.push(OutputElement::Texture(RescaleRenderElement::from_element(
                bar,
                Point::default(),
                1.0,
            )));
        }
    }
    let top_start = elements.len();
    push_layers(&mut elements, &mut anchors, renderer, Layer::Top);

    // Le finestre chiuse svaniscono sopra le altre.
    let context_id = renderer.context_id();
    for closing in context.closing {
        let progress = animation::progress(
            context.now.saturating_duration_since(closing.start),
            animation::CLOSE,
            context.animation_speed,
        );
        let texture = TextureRenderElement::from_static_texture(
            closing.id.clone(),
            context_id.clone(),
            closing.bbox.loc.to_f64(),
            closing.texture.clone(),
            1,
            Transform::Normal,
            Some(animation::close_alpha(progress)),
            None,
            Some(closing.texture.size().to_logical(1, Transform::Normal)),
            None,
            Kind::Unspecified,
        );
        elements.push(OutputElement::Texture(RescaleRenderElement::from_element(
            texture,
            closing.center,
            animation::close_scale(progress),
        )));
    }

    // Le finestre che vanno nel Dock o ne escono, sopra le altre (e sopra il Dock: la coda
    // dell'imbuto si vede dentro la cella).
    let genie_program = ossido_render::shaders(renderer).genie;
    for genie in context.genies {
        let mut commit = genie.commit.get();
        commit.increment();
        genie.commit.set(commit);
        if let Some(element) =
            ossido_render::genie::GenieElement::new(ossido_render::genie::GenieParts {
                id: genie.id.clone(),
                commit,
                context_id: context_id.clone(),
                texture: genie.texture.clone(),
                snapshot: genie.snapshot,
                window: genie.frame,
                target: genie.target,
                shape: genie.shape(context.now, context.animation_speed),
                output: output_size,
                program: genie_program.clone(),
            })
        {
            elements.insert(top_start, OutputElement::Genie(element));
            for anchor in &mut anchors {
                anchor.index = after_insert(anchor.index, top_start);
            }
        }
    }

    for window in space.elements().rev() {
        // La finestra trascinata in Panorama è già in cima.
        if context
            .panorama
            .as_ref()
            .and_then(|p| p.drag.as_ref())
            .is_some_and(|(dragged, _)| dragged == window)
        {
            continue;
        }
        // App Exposé: le finestre delle altre app spariscono.
        if let Some(panorama) = &context.panorama
            && panorama.mode == crate::panorama::Mode::App
            && panorama.progress > 0.0
            && !panorama.placements.iter().any(|p| &p.window == window)
        {
            continue;
        }
        // Una finestra che esce dal Dock si vede solo nel genio, fino alla fine.
        if context.genies.iter().any(|genie| genie.hides(window)) {
            continue;
        }
        let mut factor = open_scale(window, context);
        let mut offset = Point::<i32, Logical>::default();
        // Schermo intero in corso: la cornice di adesso va sul rettangolo dell'animazione.
        if let Some(rect) = morph_rect(window, context)
            && let Some(geometry) = space.element_geometry(window)
        {
            let (shift, scale) = fit(frame_rect(window, geometry), rect);
            offset = shift;
            factor *= scale;
        }
        // In Panorama la cornice va dalla sua posizione a quella disposta.
        if let Some(panorama) = &context.panorama
            && let Some(placement) = panorama.placements.iter().find(|p| &p.window == window)
            && placement.from.size.w > 0.0
        {
            let now = crate::panorama::interpolate(placement, panorama.progress);
            let (shift, scale) = fit(placement.from, now);
            offset = shift;
            factor *= scale;
        }
        let buttons = match &context.buttons {
            Some(view) if &view.window == window => ButtonsLook {
                symbols: view.symbols,
                pressed: view.pressed,
                alt: context.alt,
            },
            _ => ButtonsLook::default(),
        };
        let Some(at) = placed(space, window) else {
            continue;
        };
        elements.extend(window_elements(
            renderer,
            &shaders,
            at,
            window,
            output_origin,
            output_size,
            scale,
            offset,
            factor,
            buttons,
        ));
        if let Some(toplevel) = window.toplevel()
            && let Some(location) = space.element_location(window)
        {
            anchors.push(GlassAnchor {
                index: elements.len(),
                surface: toplevel.wl_surface().clone(),
                origin: (location - window.geometry().loc - output_origin + offset)
                    .to_physical_precise_round(scale),
                opacity: 1.0,
            });
        }
    }

    // Da qui in giù solo lo sfondo del desktop: è quello che vede il vetro smerigliato.
    let static_start = elements.len();
    push_layers(&mut elements, &mut anchors, renderer, Layer::Bottom);
    let background_start = elements.len();
    push_layers(&mut elements, &mut anchors, renderer, Layer::Background);

    // App Exposé: in alto lo sfondo si scurisce (sopra lo sfondo, sotto le finestre).
    if let Some(panorama) = &context.panorama
        && panorama.mode == crate::panorama::Mode::App
    {
        let width = f64::from(output_size.w) / scale;
        let shade = output.user_data().get_or_insert(AppShade::default);
        if let Some(element) = shade.0.element(
            renderer,
            "shade",
            Size::from((width, crate::panorama::APP_SHADE_HEIGHT)),
            Point::default(),
            scale,
            panorama.progress as f32,
            |canvas| {
                let rows = crate::panorama::APP_SHADE_HEIGHT as usize;
                for row in 0..rows {
                    let t = 1.0 - row as f32 / rows as f32;
                    let alpha = (crate::panorama::APP_SHADE_ALPHA * t * 255.0).round() as u8;
                    canvas.fill_rect([0.0, row as f32, width as f32, 1.0], [0, 0, 0, alpha]);
                }
            },
        ) {
            elements.insert(
                background_start,
                OutputElement::Texture(RescaleRenderElement::from_element(
                    element,
                    Point::default(),
                    1.0,
                )),
            );
            for anchor in &mut anchors {
                anchor.index = after_insert(anchor.index, background_start);
            }
        }
    }

    // La fascia nera fra le due scrivanie, sotto tutto: le due scrivanie non la coprono.
    // Prima del fondale della barra, che deve vederla dietro di sé.
    if let Some(shift) = context.space_shift {
        let gap = Rectangle::<f64, Logical>::new(
            (shift.gap, 0.0).into(),
            (
                f64::from(crate::workspaces::SPACE_GAP),
                f64::from(output_size.h) / scale,
            )
                .into(),
        )
        .to_physical_precise_round(scale);
        elements.push(OutputElement::Solid(SolidColorRenderElement::new(
            output
                .user_data()
                .get_or_insert(SpaceGapId::default)
                .0
                .clone(),
            gap,
            CommitCounter::default(),
            [0.0, 0.0, 0.0, 1.0],
            Kind::Unspecified,
        )));
    }

    // Con la barra dei menu, il gradiente che comprime le luci in cima allo sfondo.
    let has_menubar = layer_map
        .layers()
        .any(|layer| layer.namespace() == "menubar");
    if has_menubar && let Some(program) = &shaders.tone {
        let backdrop = output
            .user_data()
            .get_or_insert(ossido_render::backdrop::Backdrop::default);
        let behind = elements.get(background_start..).unwrap_or_default();
        match backdrop.element(
            renderer,
            program,
            behind,
            output_size,
            scale,
            context.clear_color,
        ) {
            Ok(tone) => {
                elements.insert(background_start, OutputElement::Tone(tone));
                for anchor in &mut anchors {
                    anchor.index = after_insert(anchor.index, background_start);
                }
            }
            Err(err) => warn!("fondale della barra saltato: {err:?}"),
        }
    }

    // Il vetro dallo sfondo verso il primo piano: un vetro più in alto vede quelli sotto.
    if let Some(programs) = &shaders.glass {
        let output_rect = Rectangle::from_size(output_size);
        for anchor in anchors.iter().rev() {
            let regions: Vec<PhysicalRegion> = current_regions(&anchor.surface)
                .iter()
                .map(|region| PhysicalRegion {
                    rect: Rectangle::new(
                        anchor.origin
                            + Point::<i32, Logical>::from((region.x, region.y))
                                .to_physical_precise_round(scale),
                        Size::<i32, Logical>::from((region.width, region.height))
                            .to_physical_precise_round(scale),
                    ),
                    radius: region.radius * scale,
                    material: match region.material {
                        Material::Clear => CLEAR_LIGHT,
                        Material::Thick => THICK_LIGHT,
                        Material::Regular => REGULAR_LIGHT,
                        Material::Notification => NOTIFICATION_LIGHT,
                    },
                    opacity: region.opacity as f32 * anchor.opacity,
                    scale,
                    tip: region.shape == Shape::Tip,
                    group: region.group,
                })
                .collect();
            if regions.is_empty() {
                continue;
            }
            let index = anchor.index.min(elements.len());
            let behind = elements.get(index..).unwrap_or_default();
            let behind_static = elements.get(index.max(static_start)..).unwrap_or_default();
            let glass = with_states(&anchor.surface, |states| {
                states.data_map.insert_if_missing(GlassCache::default);
                states.data_map.get::<GlassCache>().map(|cache| {
                    cache.elements(
                        renderer,
                        programs,
                        &regions,
                        behind,
                        behind_static,
                        context.glass_quality,
                        output_rect,
                        scale,
                        context.clear_color,
                    )
                })
            });
            match glass {
                Some(Ok(glass)) => {
                    // Le regioni dichiarate dopo stanno sopra (un sottomenu sul suo menu):
                    // nella lista, dal primo piano in giù, vengono prima.
                    let glass: Vec<OutputElement> =
                        glass.into_iter().rev().map(OutputElement::Glass).collect();
                    elements.splice(index..index, glass);
                }
                Some(Err(err)) => warn!("vetro saltato in questo frame: {err:?}"),
                None => {}
            }
        }
    }
    elements
}

#[cfg(test)]
mod tests {
    use super::after_insert;

    #[test]
    fn glass_ending_where_the_background_starts_stays_in_front() {
        // Barra dei menu senza finestre: il suo vetro va in 1 e il fondale si inserisce in 1.
        assert_eq!(after_insert(1, 1), 1);
        // Il vetro di uno strato di sfondo, dietro il fondale, scala di uno.
        assert_eq!(after_insert(3, 1), 4);
        assert_eq!(after_insert(0, 1), 0);
    }
}
