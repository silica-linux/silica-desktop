//! wlr-layer-shell: superfici della shell (barra dei menu, Dock, sfondo) ancorate
//! ai bordi dell'output, sopra o sotto le finestre.

use smithay::delegate_layer_shell;
use smithay::desktop::{LayerSurface, WindowSurfaceType, layer_map_for_output};
use smithay::output::Output;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point, Rectangle, SERIAL_COUNTER};
use smithay::wayland::compositor::with_states;
use smithay::wayland::shell::wlr_layer::{
    ExclusiveZone, KeyboardInteractivity, Layer, LayerSurface as WlrLayerSurface, LayerSurfaceData,
    WlrLayerShellHandler, WlrLayerShellState,
};
use tracing::{info, warn};

use crate::state::State;

impl WlrLayerShellHandler for State {
    fn shell_state(&mut self) -> &mut WlrLayerShellState {
        &mut self.layer_shell_state
    }

    fn new_layer_surface(
        &mut self,
        surface: WlrLayerSurface,
        output: Option<WlOutput>,
        layer: Layer,
        namespace: String,
    ) {
        let output = output
            .as_ref()
            .and_then(Output::from_resource)
            .unwrap_or_else(|| self.output.clone());
        info!("nuovo layer {namespace:?} ({layer:?})");
        let mut map = layer_map_for_output(&output);
        if let Err(err) = map.map_layer(&LayerSurface::new(surface, namespace)) {
            warn!("layer non mappato: {err}");
        }
    }

    fn layer_destroyed(&mut self, surface: WlrLayerSurface) {
        let mut map = layer_map_for_output(&self.output);
        let layer = map
            .layers()
            .find(|layer| layer.layer_surface() == &surface)
            .cloned();
        if let Some(layer) = layer {
            map.unmap_layer(&layer);
        }
    }
}

delegate_layer_shell!(State);

impl State {
    /// Al commit di un layer: ridispone i layer e manda il primo configure. Un layer
    /// con tastiera esclusiva sopra le finestre prende il focus.
    pub fn layer_commit(&mut self, surface: &WlSurface) {
        let (layer, first) = {
            let mut map = layer_map_for_output(&self.output);
            let Some(layer) = map
                .layer_for_surface(surface, WindowSurfaceType::TOPLEVEL)
                .cloned()
            else {
                return;
            };
            map.arrange();
            let sent = with_states(surface, |states| {
                states
                    .data_map
                    .get::<LayerSurfaceData>()
                    .and_then(|data| data.lock().ok())
                    .is_some_and(|data| data.initial_configure_sent)
            });
            if !sent {
                layer.layer_surface().send_configure();
            }
            (layer, !sent)
        };
        let exclusive = layer.cached_state().keyboard_interactivity
            == KeyboardInteractivity::Exclusive
            && matches!(layer.layer(), Layer::Top | Layer::Overlay);
        if first && exclusive {
            self.focus_layer(&layer);
        }
    }

    /// Il layer Top o Overlay che chiede la tastiera in esclusiva, se c'è.
    pub fn exclusive_layer_surface(&self) -> Option<WlSurface> {
        let map = layer_map_for_output(&self.output);
        map.layers()
            .rev()
            .find(|layer| {
                matches!(layer.layer(), Layer::Top | Layer::Overlay)
                    && layer.cached_state().keyboard_interactivity
                        == KeyboardInteractivity::Exclusive
            })
            .map(|layer| layer.wl_surface().clone())
    }

    pub fn focus_layer(&mut self, layer: &LayerSurface) {
        let surface = layer.wl_surface().clone();
        if let Some(keyboard) = self.seat.get_keyboard() {
            keyboard.set_focus(self, Some(surface), SERIAL_COUNTER.next_serial());
        }
    }

    /// Il layer di quel livello sotto un punto, con la sua posizione.
    pub fn layer_under(
        &self,
        layers: &[Layer],
        pos: Point<f64, Logical>,
    ) -> Option<(LayerSurface, Point<i32, Logical>)> {
        let map = layer_map_for_output(&self.output);
        let origin = self.space.output_geometry(&self.output)?.loc;
        layers.iter().find_map(|level| {
            let layer = map.layer_under(*level, pos - origin.to_f64())?;
            let geometry = map.layer_geometry(layer)?;
            Some((layer.clone(), geometry.loc + origin))
        })
    }

    pub fn layer_list(&self) -> Vec<silica_control::commands::LayerInfo> {
        let focus = self
            .seat
            .get_keyboard()
            .and_then(|keyboard| keyboard.current_focus());
        let map = layer_map_for_output(&self.output);
        let origin = self
            .space
            .output_geometry(&self.output)
            .map(|g| g.loc)
            .unwrap_or_default();
        map.layers()
            .filter_map(|layer| {
                let geometry = map.layer_geometry(layer)?;
                let state = layer.cached_state();
                Some(silica_control::commands::LayerInfo {
                    namespace: layer.namespace().to_owned(),
                    layer: match layer.layer() {
                        Layer::Background => "background",
                        Layer::Bottom => "bottom",
                        Layer::Top => "top",
                        Layer::Overlay => "overlay",
                    }
                    .to_owned(),
                    x: geometry.loc.x + origin.x,
                    y: geometry.loc.y + origin.y,
                    width: geometry.size.w,
                    height: geometry.size.h,
                    exclusive_zone: match state.exclusive_zone {
                        ExclusiveZone::Exclusive(zone) => i32::try_from(zone).unwrap_or(i32::MAX),
                        ExclusiveZone::Neutral => 0,
                        ExclusiveZone::DontCare => -1,
                    },
                    focused: focus.as_ref() == Some(layer.wl_surface()),
                })
            })
            .collect()
    }

    /// L'area libera dai layer con zona esclusiva: lì vanno le finestre.
    pub fn usable_area(&self) -> Rectangle<i32, Logical> {
        let map = layer_map_for_output(&self.output);
        let mut zone = map.non_exclusive_zone();
        if let Some(geometry) = self.space.output_geometry(&self.output) {
            zone.loc += geometry.loc;
        }
        zone
    }
}
