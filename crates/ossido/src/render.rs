//! La lista degli elementi di un output con GLES: layer, finestre ed effetti di
//! `ossido-render`, dal primo piano allo sfondo. Con pixman (headless senza EGL) si usa
//! ancora `space_render_elements`, senza effetti.

use ossido_render::corners::RoundedSurface;
use ossido_render::glass::{CLEAR_LIGHT, GlassCache, GlassElement, PhysicalRegion, THICK_LIGHT};
use ossido_render::shadow::{ACTIVE_LIGHT, INACTIVE_LIGHT, WindowShadow};
use ossido_render::titlebar::{Titlebar, TitlebarState};
use smithay::backend::renderer::element::AsRenderElements;
use smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::renderer::gles::element::PixelShaderElement;
use smithay::desktop::{LayerSurface, Space, Window, layer_map_for_output};
use smithay::output::Output;
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::render_elements;
use smithay::utils::{Physical, Point, Rectangle, Size};
use smithay::wayland::compositor::with_states;
use smithay::wayland::shell::wlr_layer::Layer;
use tracing::warn;

use crate::decorations;
use crate::glass::{Material, current_regions};

render_elements! {
    pub OutputElement<=GlesRenderer>;
    Surface=WaylandSurfaceRenderElement<GlesRenderer>,
    Rounded=RoundedSurface,
    /// Elementi a pixel shader: ombre e barre del titolo.
    Shadow=PixelShaderElement,
    Glass=GlassElement,
}

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

/// Gli elementi dell'output, dal primo piano allo sfondo.
pub fn output_elements(
    renderer: &mut GlesRenderer,
    space: &Space<Window>,
    output: &Output,
    titlebar_hover: Option<&Window>,
    clear_color: [f32; 4],
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

    for window in space.elements().rev() {
        let (Some(location), Some(geometry)) = (
            space.element_location(window),
            space.element_geometry(window),
        ) else {
            continue;
        };
        let render_location =
            (location - window.geometry().loc - output_origin).to_physical_precise_round(scale);
        let surfaces = window.render_elements::<WaylandSurfaceRenderElement<GlesRenderer>>(
            renderer,
            render_location,
            scale.into(),
            1.0,
        );
        let anchor = window.toplevel().map(|toplevel| GlassAnchor {
            index: 0,
            surface: toplevel.wl_surface().clone(),
            origin: render_location,
        });
        // A schermo intero niente angoli, barra né ombra (come nelle riprese).
        if crate::windows::is_fullscreen(window) {
            elements.extend(surfaces.into_iter().map(OutputElement::Surface));
            anchors.extend(anchor.map(|anchor| GlassAnchor {
                index: elements.len(),
                ..anchor
            }));
            continue;
        }
        let mut frame = decorations::frame_of(window, geometry);
        frame.loc -= output_origin;
        let frame_physical = frame.to_f64().to_physical(scale);
        elements.extend(surfaces.into_iter().map(|surface| {
            OutputElement::Rounded(RoundedSurface::new(
                surface,
                frame_physical,
                scale,
                output_size,
                shaders.corners.clone(),
            ))
        }));
        let active = is_active(window);
        if decorations::is_server_side(window)
            && let Some(program) = &shaders.titlebar
        {
            window.user_data().insert_if_missing(Titlebar::default);
            if let Some(titlebar) = window.user_data().get::<Titlebar>() {
                let state = TitlebarState {
                    active,
                    hovered: titlebar_hover == Some(window),
                    scale,
                };
                elements.push(OutputElement::Shadow(
                    titlebar.element(program, frame, state),
                ));
            }
        }
        // L'ombra va dietro la finestra.
        if let Some(program) = &shaders.shadow {
            let style = if active { ACTIVE_LIGHT } else { INACTIVE_LIGHT };
            window.user_data().insert_if_missing(WindowShadow::default);
            if let Some(shadow) = window.user_data().get::<WindowShadow>() {
                elements.push(OutputElement::Shadow(shadow.element(program, frame, style)));
            }
        }
        // Il vetro della finestra va dietro le sue superfici ma non vede la sua barra né
        // la sua ombra (che dentro la cornice è comunque nascosta).
        anchors.extend(anchor.map(|anchor| GlassAnchor {
            index: elements.len(),
            ..anchor
        }));
    }

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
                            + Point::<i32, smithay::utils::Logical>::from((region.x, region.y))
                                .to_physical_precise_round(scale),
                        Size::<i32, smithay::utils::Logical>::from((region.width, region.height))
                            .to_physical_precise_round(scale),
                    ),
                    radius: region.radius * scale,
                    material: match region.material {
                        Material::Clear => CLEAR_LIGHT,
                        Material::Thick => THICK_LIGHT,
                    },
                    scale,
                })
                .collect();
            if regions.is_empty() {
                continue;
            }
            let index = anchor.index.min(elements.len());
            let behind = elements.get(index..).unwrap_or_default();
            let glass = with_states(&anchor.surface, |states| {
                states.data_map.insert_if_missing(GlassCache::default);
                states.data_map.get::<GlassCache>().map(|cache| {
                    cache.elements(
                        renderer,
                        programs,
                        &regions,
                        behind,
                        output_rect,
                        scale,
                        clear_color,
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
