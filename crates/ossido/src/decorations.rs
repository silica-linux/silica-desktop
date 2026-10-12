//! Decorazioni lato server (`xdg-decoration`): la barra del titolo di Ossido sopra il
//! contenuto della finestra, con i pulsanti chiudi, riduci e schermo intero.
//!
//! La posizione della finestra nello spazio resta quella del contenuto; la cornice è il
//! contenuto più la barra, sopra. Angoli e ombra seguono la cornice.

use ossido_render::titlebar::{self, Part};
use smithay::delegate_xdg_decoration;
use smithay::desktop::Window;
use smithay::desktop::space::SpaceElement;
use smithay::reexports::wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode;
use smithay::utils::{Logical, Point, Rectangle, Size};
use smithay::wayland::shell::xdg::ToplevelSurface;
use smithay::wayland::shell::xdg::decoration::XdgDecorationHandler;
use tracing::{info, warn};

use crate::state::State;

/// La finestra ha la barra di Ossido: modo lato server già confermato al client, e non è
/// a schermo intero (lì la barra sparisce, come nelle riprese).
pub fn is_server_side(window: &Window) -> bool {
    window
        .toplevel()
        .is_some_and(|toplevel| toplevel.current_state().decoration_mode == Some(Mode::ServerSide))
        && !crate::windows::is_fullscreen(window)
}

/// Altezza della barra sopra il contenuto (0 senza decorazioni lato server).
pub fn titlebar_height(window: &Window) -> i32 {
    if is_server_side(window) {
        titlebar::HEIGHT
    } else {
        0
    }
}

/// La cornice dato il rettangolo del contenuto: la barra si aggiunge sopra.
pub fn frame_of(window: &Window, content: Rectangle<i32, Logical>) -> Rectangle<i32, Logical> {
    let bar = titlebar_height(window);
    Rectangle::new(
        content.loc - Point::from((0, bar)),
        content.size + Size::from((0, bar)),
    )
}

/// Dove il puntatore è sopra una finestra: barra (con la parte) o contenuto.
#[derive(Debug, Clone, PartialEq)]
pub enum Hit {
    Titlebar(Window, Part, Point<f64, Logical>),
    Content(Window),
}

impl XdgDecorationHandler for State {
    fn new_decoration(&mut self, toplevel: ToplevelSurface) {
        // Ossido preferisce disegnare la barra: aspetto uniforme fra le app.
        toplevel.with_pending_state(|state| state.decoration_mode = Some(Mode::ServerSide));
        if toplevel.is_initial_configure_sent() {
            toplevel.send_pending_configure();
        }
    }

    fn request_mode(&mut self, toplevel: ToplevelSurface, mode: Mode) {
        // Un client che disegna da sé la barra (GTK4) la tiene.
        toplevel.with_pending_state(|state| state.decoration_mode = Some(mode));
        if toplevel.is_initial_configure_sent() {
            toplevel.send_pending_configure();
        }
    }

    fn unset_mode(&mut self, toplevel: ToplevelSurface) {
        toplevel.with_pending_state(|state| state.decoration_mode = Some(Mode::ServerSide));
        if toplevel.is_initial_configure_sent() {
            toplevel.send_pending_configure();
        }
    }
}

delegate_xdg_decoration!(State);

impl State {
    /// La cornice di una finestra nello spazio, se è mappata.
    pub fn window_frame(&self, window: &Window) -> Option<Rectangle<i32, Logical>> {
        self.space
            .element_geometry(window)
            .map(|content| frame_of(window, content))
    }

    /// La finestra sotto un punto, dall'alto, con la barra contata come parte della finestra.
    pub fn window_hit(&self, location: Point<f64, Logical>) -> Option<Hit> {
        for window in self.space.elements().rev() {
            if let Some(frame) = self.window_frame(window)
                && is_server_side(window)
            {
                let local = location - frame.loc.to_f64();
                if frame.to_f64().contains(location)
                    && let Some(part) = titlebar::part_at(local)
                {
                    return Some(Hit::Titlebar(window.clone(), part, local));
                }
            }
            let Some(render_location) = self
                .space
                .element_location(window)
                .map(|loc| loc - window.geometry().loc)
            else {
                continue;
            };
            if window.is_in_input_region(&(location - render_location.to_f64())) {
                return Some(Hit::Content(window.clone()));
            }
        }
        None
    }

    /// Una finestra appena passata alle decorazioni lato server: se la barra uscirebbe
    /// dall'area utile, la finestra scende.
    pub fn make_room_for_titlebar(&mut self, window: &Window) {
        let (Some(location), Some(frame)) = (
            self.space.element_location(window),
            self.window_frame(window),
        ) else {
            return;
        };
        let top = self.usable_area().loc.y;
        if frame.loc.y < top {
            let shift = top.saturating_sub(frame.loc.y);
            self.space
                .map_element(window.clone(), location + Point::from((0, shift)), false);
        }
    }

    /// Esegue il pulsante della barra rilasciato sopra lo stesso pulsante.
    pub fn titlebar_action(&mut self, window: &Window, part: Part) {
        let result = match part {
            Part::Close => {
                if let Some(toplevel) = window.toplevel() {
                    toplevel.send_close();
                }
                Ok(())
            }
            Part::Minimize => self.minimize_window(window),
            Part::Zoom => self.toggle_fullscreen(window),
            Part::Bar => Ok(()),
        };
        if let Err(err) = result {
            warn!("pulsante {part:?}: {err}");
        } else {
            info!("pulsante {part:?}");
        }
    }
}
