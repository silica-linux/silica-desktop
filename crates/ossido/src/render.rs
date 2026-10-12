//! La lista degli elementi di un output con GLES: layer, finestre ed effetti di
//! `ossido-render`, dal primo piano allo sfondo. Con pixman (headless senza EGL) si usa
//! ancora `space_render_elements`, senza effetti.

use ossido_render::corners::RoundedSurface;
use ossido_render::shadow::{ACTIVE_LIGHT, INACTIVE_LIGHT, WindowShadow};
use smithay::backend::renderer::element::AsRenderElements;
use smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::renderer::gles::element::PixelShaderElement;
use smithay::desktop::{LayerSurface, Space, Window, layer_map_for_output};
use smithay::output::Output;
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::render_elements;
use smithay::wayland::shell::wlr_layer::Layer;

render_elements! {
    pub OutputElement<=GlesRenderer>;
    Surface=WaylandSurfaceRenderElement<GlesRenderer>,
    Rounded=RoundedSurface,
    Shadow=PixelShaderElement,
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

    let layer_map = layer_map_for_output(output);
    let push_layers = |elements: &mut Vec<OutputElement>, renderer: &mut GlesRenderer, level| {
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
        }
    };
    push_layers(&mut elements, renderer, Layer::Overlay);
    push_layers(&mut elements, renderer, Layer::Top);

    for window in space.elements().rev() {
        let (Some(location), Some(geometry)) = (
            space.element_location(window),
            space.element_geometry(window),
        ) else {
            continue;
        };
        let render_location =
            (location - window.geometry().loc - output_origin).to_physical_precise_round(scale);
        let mut window_rect = geometry.to_f64();
        window_rect.loc -= output_origin.to_f64();
        let window_rect = window_rect.to_physical(scale);
        let surfaces = window.render_elements::<WaylandSurfaceRenderElement<GlesRenderer>>(
            renderer,
            render_location,
            scale.into(),
            1.0,
        );
        elements.extend(surfaces.into_iter().map(|surface| {
            OutputElement::Rounded(RoundedSurface::new(
                surface,
                window_rect,
                scale,
                output_size,
                shaders.corners.clone(),
            ))
        }));
        // L'ombra va dietro le superfici della finestra.
        if let Some(program) = &shaders.shadow {
            let style = if is_active(window) {
                ACTIVE_LIGHT
            } else {
                INACTIVE_LIGHT
            };
            window.user_data().insert_if_missing(WindowShadow::default);
            if let Some(shadow) = window.user_data().get::<WindowShadow>() {
                let mut logical = geometry;
                logical.loc -= output_origin;
                elements.push(OutputElement::Shadow(
                    shadow.element(program, logical, style),
                ));
            }
        }
    }

    push_layers(&mut elements, renderer, Layer::Bottom);
    push_layers(&mut elements, renderer, Layer::Background);
    elements
}
