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
    /// Toglie la finestra dallo schermo e dà il focus a quella sotto.
    pub fn minimize_window(&mut self, window: &Window) -> Result<(), String> {
        let location = self
            .space
            .element_location(window)
            .ok_or("finestra non sullo schermo")?;
        self.space.unmap_elem(window);
        self.minimized.push((window.clone(), location));
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
        self.space.map_element(window, location, true);
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
