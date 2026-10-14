//! Finestre ridotte e a schermo intero.

use std::cell::Cell;

use smithay::desktop::Window;
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::utils::{Logical, Point, Rectangle};

use crate::state::State;

/// Dove era la finestra prima dello schermo intero, nei suoi dati.
#[derive(Default)]
struct BeforeFullscreen(Cell<Option<Rectangle<i32, Logical>>>);

pub fn is_fullscreen(window: &Window) -> bool {
    window.toplevel().is_some_and(|toplevel| {
        toplevel
            .current_state()
            .states
            .contains(xdg_toplevel::State::Fullscreen)
    })
}

impl State {
    /// Dove finisce la miniatura di una finestra ridotta, in punti dell'output: la cella prima
    /// del Cestino nel Dock (il pannello di vetro più largo del layer `dock`). Dalla ripresa
    /// della riduzione (`windows.md`): il Dock si allarga di una cella (816–1743 → 784–1775) e
    /// la miniatura si ferma centrata a x 1672, cioè 71 pt a sinistra del bordo destro di prima;
    /// in verticale al centro del Dock. Miniatura larga 57 e alta al più 47 (`dock.md`, D-08).
    /// Senza Dock: in basso al centro dello schermo.
    pub fn minimize_target(&self, window: &Window) -> Option<Rectangle<f64, Logical>> {
        const FROM_RIGHT: f64 = 71.0;
        const WIDTH: f64 = 57.0;
        const MAX_HEIGHT: f64 = 47.0;
        let geometry = self.space.element_geometry(window)?;
        let frame = crate::decorations::frame_of(window, geometry).to_f64();
        let height = if frame.size.w > 0.0 {
            (WIDTH * frame.size.h / frame.size.w).min(MAX_HEIGHT)
        } else {
            MAX_HEIGHT
        };
        let map = smithay::desktop::layer_map_for_output(&self.output);
        let panel = map
            .layers()
            .filter(|layer| layer.namespace() == "dock")
            .filter_map(|layer| {
                let origin = map.layer_geometry(layer)?.loc.to_f64();
                crate::glass::current_regions(layer.wl_surface())
                    .into_iter()
                    .map(|r| {
                        Rectangle::<f64, Logical>::new(
                            (origin.x + f64::from(r.x), origin.y + f64::from(r.y)).into(),
                            (f64::from(r.width), f64::from(r.height)).into(),
                        )
                    })
                    .max_by(|a, b| a.size.w.total_cmp(&b.size.w))
            })
            .next();
        let output = self.space.output_geometry(&self.output)?.to_f64();
        let center: Point<f64, Logical> = match panel {
            Some(panel) => (
                panel.loc.x + panel.size.w - FROM_RIGHT,
                panel.loc.y + panel.size.h / 2.0,
            )
                .into(),
            None => (
                output.loc.x + output.size.w / 2.0,
                output.loc.y + output.size.h - 40.0,
            )
                .into(),
        };
        Some(Rectangle::new(
            (center.x - WIDTH / 2.0, center.y - height / 2.0).into(),
            (WIDTH, height).into(),
        ))
    }

    /// Toglie la finestra dallo schermo e dà il focus a quella sotto.
    pub fn minimize_window(&mut self, window: &Window) -> Result<(), String> {
        let location = self
            .space
            .element_location(window)
            .ok_or("finestra non sullo schermo")?;
        // La miniatura per il Dock e il genio, finché la finestra è ancora sullo schermo.
        self.capture_thumbnail(window);
        self.start_genie(window, false);
        self.space.unmap_elem(window);
        self.minimized.push((window.clone(), location));
        self.announce_minimized(window, true);
        let next = self.space.elements().last().cloned();
        self.focus_window(next);
        Ok(())
    }

    /// Riporta una finestra ridotta dov'era. Falso se non era ridotta.
    pub fn restore_window(&mut self, window: &Window) -> bool {
        let Some(index) = self.minimized.iter().position(|(w, _)| w == window) else {
            return false;
        };
        let (window, location) = self.minimized.remove(index);
        self.announce_minimized(&window, false);
        self.space.map_element(window.clone(), location, true);
        // Esce dal Dock a genio; finché il genio non finisce la finestra resta nascosta.
        self.start_genie(&window, true);
        true
    }

    /// Schermo intero sull'output, o ritorno alla geometria di prima.
    pub fn toggle_fullscreen(&mut self, window: &Window) -> Result<(), String> {
        let toplevel = window.toplevel().ok_or("finestra senza toplevel")?;
        window
            .user_data()
            .insert_if_missing(BeforeFullscreen::default);
        let before = window
            .user_data()
            .get::<BeforeFullscreen>()
            .ok_or("stato della finestra assente")?;
        if is_fullscreen(window) {
            let previous = before.0.take();
            toplevel.with_pending_state(|state| {
                state.states.unset(xdg_toplevel::State::Fullscreen);
                state.size = previous.map(|rect| rect.size);
            });
            toplevel.send_pending_configure();
            if let Some(previous) = previous {
                self.space.map_element(window.clone(), previous.loc, true);
            }
        } else {
            let output = self
                .space
                .output_geometry(&self.output)
                .ok_or("output senza geometria")?;
            let current = self
                .space
                .element_geometry(window)
                .ok_or("finestra non sullo schermo")?;
            before.0.set(Some(current));
            toplevel.with_pending_state(|state| {
                state.states.set(xdg_toplevel::State::Fullscreen);
                state.size = Some(output.size);
            });
            toplevel.send_pending_configure();
            self.space.map_element(window.clone(), output.loc, true);
        }
        Ok(())
    }

    /// Una finestra ridotta con quell'id.
    pub fn minimized_window(&self, id: u64) -> Option<Window> {
        self.minimized
            .iter()
            .find(|(window, _)| Self::window_id(window) == Some(id))
            .map(|(window, _)| window.clone())
    }
}

/// Posizione salvata di una finestra ridotta.
pub type Minimized = (Window, Point<i32, Logical>);
