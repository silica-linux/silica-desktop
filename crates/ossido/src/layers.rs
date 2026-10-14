//! wlr-layer-shell: shell surfaces (menu bar, Dock, wallpaper) anchored to the output
//! edges, above or below the windows.

use smithay::delegate_layer_shell;
use smithay::desktop::{LayerSurface, Window, WindowSurfaceType, layer_map_for_output};
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

/// The namespace of the file manager's desktop surface (#147): `desktop-icons:<app id>`, on the
/// bottom layer (above the wallpaper, below the windows). On the reference the desktop icons are
/// a window of the file manager, and a click on the desktop makes it the frontmost app.
pub const DESKTOP_ICONS: &str = "desktop-icons";

/// The owner app of a desktop surface's namespace: `Some(app id)` for `desktop-icons:<app id>`,
/// `Some("")` for a bare `desktop-icons`, `None` for any other surface.
pub fn desktop_owner(namespace: &str) -> Option<&str> {
    let rest = namespace.strip_prefix(DESKTOP_ICONS)?;
    if rest.is_empty() {
        return Some("");
    }
    rest.strip_prefix(':')
}

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
        info!("new layer {namespace:?} ({layer:?})");
        let mut map = layer_map_for_output(&output);
        if let Err(err) = map.map_layer(&LayerSurface::new(surface, namespace)) {
            warn!("layer not mapped: {err}");
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
    /// On a layer's commit: rearranges the layers and sends the first configure. A layer
    /// with exclusive keyboard above the windows takes the focus.
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
        // A layer may ask for the keyboard after the first commit (Faro opening) and
        // release it (Faro closing): the transition counts, not only the start.
        layer
            .user_data()
            .insert_if_missing(|| WasExclusive(std::cell::Cell::new(false)));
        let was = layer
            .user_data()
            .get::<WasExclusive>()
            .is_some_and(|flag| flag.0.replace(exclusive));
        if first && desktop_owner(layer.namespace()).is_some() {
            self.desktop_owner_fallback(None);
        }
        if exclusive && (first || !was) {
            self.focus_layer(&layer);
        } else if was && !exclusive {
            let focused = self
                .seat
                .get_keyboard()
                .and_then(|keyboard| keyboard.current_focus());
            if focused.as_ref() == Some(layer.wl_surface()) {
                let top = self.space.elements().last().cloned();
                self.focus_window(top);
            }
        }
    }

    /// The Top or Overlay layer asking for the keyboard exclusively, if any.
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

    /// The layer of that level under a point, with its position.
    pub fn layer_under(
        &self,
        layers: &[Layer],
        pos: Point<f64, Logical>,
    ) -> Option<(LayerSurface, Point<i32, Logical>)> {
        let map = layer_map_for_output(&self.output);
        let origin = self.space.output_geometry(&self.output)?.loc;
        // The input region counts, not the rectangle: a hidden shell component (a closed
        // Faro) is transparent and full screen but with an empty region.
        layers.iter().find_map(|level| {
            map.layers_on(*level).rev().find_map(|layer| {
                let geometry = map.layer_geometry(layer)?;
                let local = pos - (geometry.loc + origin).to_f64();
                layer
                    .surface_under(local, smithay::desktop::WindowSurfaceType::ALL)
                    .map(|_| (layer.clone(), geometry.loc + origin))
            })
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

    /// The file manager's desktop surface, if one is mapped, with its owner app id.
    pub fn desktop_layer(&self) -> Option<(LayerSurface, String)> {
        let map = layer_map_for_output(&self.output);
        [Layer::Bottom, Layer::Background].iter().find_map(|level| {
            map.layers_on(*level).find_map(|layer| {
                desktop_owner(layer.namespace()).map(|owner| (layer.clone(), owner.to_owned()))
            })
        })
    }

    /// With no app frontmost (no focused window other than `closing`, no windowless app) the
    /// desktop's owner is, as the Finder on macOS (#147). A window in use is never displaced.
    pub fn desktop_owner_fallback(&mut self, closing: Option<&Window>) {
        if self.windowless_front.is_some() {
            return;
        }
        if self.focused_window().is_some_and(|w| Some(&w) != closing) {
            return;
        }
        if let Some((_, owner)) = self.desktop_layer()
            && !owner.is_empty()
        {
            self.set_windowless_front(Some(owner));
        }
    }

    /// A press on the desktop (no layer above, no window): the windows lose the focus and the
    /// desktop's owner becomes the frontmost app, with the keyboard (#147). Without a desktop
    /// owner no app stays frontmost.
    pub fn desktop_pressed(&mut self) {
        self.focus_window(None);
        match self.desktop_layer() {
            Some((layer, owner)) => {
                self.set_windowless_front((!owner.is_empty()).then_some(owner));
                if layer.can_receive_keyboard_focus() {
                    self.focus_layer(&layer);
                }
            }
            None => self.set_windowless_front(None),
        }
    }

    /// The area free of layers with an exclusive zone: the windows go there.
    pub fn usable_area(&self) -> Rectangle<i32, Logical> {
        let map = layer_map_for_output(&self.output);
        let mut zone = map.non_exclusive_zone();
        if let Some(geometry) = self.space.output_geometry(&self.output) {
            zone.loc += geometry.loc;
        }
        zone
    }
}

/// Whether the layer asked for the keyboard exclusively on the previous commit.
struct WasExclusive(std::cell::Cell<bool>);

#[cfg(test)]
mod tests {
    use super::desktop_owner;

    #[test]
    fn the_desktop_surface_names_its_owner() {
        assert_eq!(
            desktop_owner("desktop-icons:dev.frascella.Mustache"),
            Some("dev.frascella.Mustache")
        );
        assert_eq!(desktop_owner("desktop-icons"), Some(""));
        // An empty owner is a desktop, but no app to bring to the front.
        assert_eq!(desktop_owner("desktop-icons:"), Some(""));
        assert_eq!(desktop_owner("wallpaper"), None);
        assert_eq!(desktop_owner("desktop-iconsx"), None);
    }
}
