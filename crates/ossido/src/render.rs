//! La lista degli elementi di un output con GLES: layer, finestre ed effetti di
//! `ossido-render`, dal primo piano allo sfondo. Con pixman (headless senza EGL) si usa
//! ancora `space_render_elements`, senza effetti.

use std::cell::Cell;
use std::time::Instant;

use ossido_render::animation;
use ossido_render::corners::{RoundedSurface, WindowSurface};
use ossido_render::glass::{
    CLEAR_LIGHT, GlassCache, GlassElement, GlassQuality, PhysicalRegion, REGULAR_LIGHT,
    THICK_LIGHT, render_to_texture,
};
use ossido_render::shadow::{ACTIVE_LIGHT, INACTIVE_LIGHT, WindowShadow};
use ossido_render::title::Title;
use ossido_render::titlebar::{Titlebar, TitlebarState};
use smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement;
use smithay::backend::renderer::element::texture::TextureRenderElement;
use smithay::backend::renderer::element::utils::RescaleRenderElement;
use smithay::backend::renderer::element::{AsRenderElements, Id, Kind};
use smithay::backend::renderer::gles::element::PixelShaderElement;
use smithay::backend::renderer::gles::{GlesRenderer, GlesTexture};
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
use crate::glass::{Material, current_regions};

render_elements! {
    pub OutputElement<=GlesRenderer>;
    Surface=WaylandSurfaceRenderElement<GlesRenderer>,
    Rounded=RoundedSurface,
    /// Elementi a pixel shader della finestra: barra del titolo e ombra.
    Shader=RescaleRenderElement<PixelShaderElement>,
    Glass=GlassElement,
    /// Texture: titoli delle barre, istantanee delle finestre che si chiudono.
    Texture=RescaleRenderElement<TextureRenderElement<GlesTexture>>,
}

/// Quello che serve al frame oltre allo spazio.
pub struct RenderContext<'a> {
    /// Finestra con il puntatore sui pulsanti della barra.
    pub titlebar_hover: Option<&'a Window>,
    pub clear_color: [f32; 4],
    /// Moltiplicatore delle durate delle animazioni (0: nessuna animazione).
    pub animation_speed: f64,
    pub closing: &'a [Closing],
    pub now: Instant,
    pub glass_quality: GlassQuality,
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

/// Quando la finestra ha mostrato il primo buffer: l'apertura parte da lì.
#[derive(Default)]
struct Opened(Cell<Option<Instant>>);

/// Dove va il vetro di una superficie: subito dietro le sue superfici nella lista.
struct GlassAnchor {
    index: usize,
    surface: WlSurface,
    /// Origine della superficie nell'output, in pixel fisici.
    origin: Point<i32, Physical>,
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

/// Gli elementi di una finestra, dal primo piano allo sfondo (superfici, barra, ombra),
/// scalati di `factor` attorno al centro della cornice. Con il vetro, l'indice dopo il
/// quale va (dietro le superfici, senza vedere barra e ombra della finestra).
#[allow(clippy::too_many_arguments)]
fn window_elements(
    renderer: &mut GlesRenderer,
    shaders: &ossido_render::Shaders,
    space: &Space<Window>,
    window: &Window,
    output_origin: Point<i32, Logical>,
    output_size: Size<i32, Physical>,
    scale: f64,
    factor: f64,
    hovered: bool,
) -> Vec<OutputElement> {
    let (Some(location), Some(geometry)) = (
        space.element_location(window),
        space.element_geometry(window),
    ) else {
        return Vec::new();
    };
    let render_location =
        (location - window.geometry().loc - output_origin).to_physical_precise_round(scale);
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
        window.user_data().insert_if_missing(Title::default);
        if let Some(title) = window.user_data().get::<Title>()
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
                hovered,
                scale,
            };
            elements.push(OutputElement::Shader(RescaleRenderElement::from_element(
                titlebar.element(program, frame, state),
                center,
                factor,
            )));
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
        space,
        window,
        output_origin,
        output_size,
        scale,
        1.0,
        false,
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
            let location = geometry.loc.to_physical_precise_round(scale);
            elements.extend(
                AsRenderElements::<GlesRenderer>::render_elements::<
                    WaylandSurfaceRenderElement<GlesRenderer>,
                >(
                    layer as &LayerSurface,
                    renderer,
                    location,
                    scale.into(),
                    1.0,
                )
                .into_iter()
                .map(OutputElement::Surface),
            );
            anchors.push(GlassAnchor {
                index: elements.len(),
                surface: layer.wl_surface().clone(),
                origin: location,
            });
        }
    };
    push_layers(&mut elements, &mut anchors, renderer, Layer::Overlay);
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

    for window in space.elements().rev() {
        let factor = open_scale(window, context);
        let hovered = context.titlebar_hover == Some(window);
        elements.extend(window_elements(
            renderer,
            &shaders,
            space,
            window,
            output_origin,
            output_size,
            scale,
            factor,
            hovered,
        ));
        if let Some(toplevel) = window.toplevel()
            && let Some(location) = space.element_location(window)
        {
            anchors.push(GlassAnchor {
                index: elements.len(),
                surface: toplevel.wl_surface().clone(),
                origin: (location - window.geometry().loc - output_origin)
                    .to_physical_precise_round(scale),
            });
        }
    }

    // Da qui in giù solo lo sfondo del desktop: è quello che vede il vetro smerigliato.
    let static_start = elements.len();
    push_layers(&mut elements, &mut anchors, renderer, Layer::Bottom);
    push_layers(&mut elements, &mut anchors, renderer, Layer::Background);

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
                    },
                    scale,
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
                    let glass: Vec<OutputElement> =
                        glass.into_iter().map(OutputElement::Glass).collect();
                    elements.splice(index..index, glass);
                }
                Some(Err(err)) => warn!("vetro saltato in questo frame: {err:?}"),
                None => {}
            }
        }
    }
    elements
}
