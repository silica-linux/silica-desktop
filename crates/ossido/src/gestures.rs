//! Gesti del trackpad (M5.6): scorrimento a tre o quattro dita fra gli spazi, in alto per
//! Panorama, in basso per le finestre dell'app (o per chiudere la vista aperta); pizzico con
//! pollice e tre dita: allargando Mostra scrivania, stringendo il launcher.
//!
//! Logica provvisoria: le soglie e il gesto che segue le dita (lo spazio che scorre insieme
//! alle dita, Panorama che si apre a metà) aspettano le misure PA-20…PA-22. Qui il gesto si
//! decide alla fine, come una scorciatoia. Arrivano dagli eventi di libinput o da
//! `input.gesture`.

use silica_control::commands::{
    DESKTOP_SHOW, LAUNCHER_TOGGLE, PANORAMA_APP, PANORAMA_TOGGLE, SPACE_NEXT, SPACE_PREVIOUS,
};

use crate::panorama::Mode;
use crate::state::State;

/// Spostamento minimo (in punti, come lo riporta libinput) per uno scorrimento.
pub const SWIPE_THRESHOLD: f64 = 80.0;
/// Scala oltre la quale un pizzico conta (allargando, o stringendo sotto l'inverso).
pub const PINCH_THRESHOLD: f64 = 1.3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GestureKind {
    Swipe,
    Pinch,
}

/// Un gesto in corso: dita e movimento accumulato.
#[derive(Clone, Copy, Debug)]
pub struct Gesture {
    pub kind: GestureKind,
    pub fingers: u32,
    pub dx: f64,
    pub dy: f64,
    pub scale: f64,
}

/// Il comando di un gesto finito, se ce n'è uno; `view` è la vista aperta.
pub fn command_for(gesture: &Gesture, view: Option<Mode>) -> Option<&'static str> {
    match gesture.kind {
        GestureKind::Swipe if gesture.fingers >= 3 => {
            let (dx, dy) = (gesture.dx, gesture.dy);
            if dx.abs() >= dy.abs() {
                if dx.abs() < SWIPE_THRESHOLD || view.is_some() {
                    return None;
                }
                // Le dita verso sinistra portano lo spazio a destra.
                Some(if dx < 0.0 { SPACE_NEXT } else { SPACE_PREVIOUS })
            } else if dy.abs() < SWIPE_THRESHOLD {
                None
            } else if dy < 0.0 {
                // In alto: Panorama (già aperto, niente).
                (view != Some(Mode::Panorama)).then_some(PANORAMA_TOGGLE)
            } else {
                // In basso: chiude la vista aperta, altrimenti le finestre dell'app.
                Some(match view {
                    Some(Mode::Panorama) => PANORAMA_TOGGLE,
                    Some(Mode::Desktop) => DESKTOP_SHOW,
                    // Aperte le finestre dell'app, lo stesso comando le chiude.
                    Some(Mode::App) | None => PANORAMA_APP,
                })
            }
        }
        GestureKind::Pinch if gesture.fingers >= 4 => {
            if gesture.scale >= PINCH_THRESHOLD {
                Some(DESKTOP_SHOW)
            } else if gesture.scale > 0.0 && gesture.scale <= 1.0 / PINCH_THRESHOLD {
                Some(LAUNCHER_TOGGLE)
            } else {
                None
            }
        }
        _ => None,
    }
}

impl State {
    pub fn gesture_begin(&mut self, kind: GestureKind, fingers: u32) {
        self.gesture = Some(Gesture {
            kind,
            fingers,
            dx: 0.0,
            dy: 0.0,
            scale: 1.0,
        });
    }

    /// Movimento (scorrimento) o scala assoluta (pizzico) dall'inizio del gesto.
    pub fn gesture_update(&mut self, dx: f64, dy: f64, scale: Option<f64>) {
        if let Some(gesture) = self.gesture.as_mut() {
            if dx.is_finite() && dy.is_finite() {
                gesture.dx += dx;
                gesture.dy += dy;
            }
            if let Some(scale) = scale.filter(|s| s.is_finite()) {
                gesture.scale = scale;
            }
        }
    }

    /// Fine del gesto: esegue il suo comando (se non annullato) e lo restituisce.
    pub fn gesture_end(&mut self, cancelled: bool) -> Option<&'static str> {
        let gesture = self.gesture.take()?;
        if cancelled {
            return None;
        }
        let id = command_for(&gesture, self.panorama_mode())?;
        if let Err(err) = self.execute_command(id, serde_json::Value::Null) {
            tracing::warn!("gesto {id}: {err}");
        }
        Some(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn swipe(fingers: u32, dx: f64, dy: f64) -> Gesture {
        Gesture {
            kind: GestureKind::Swipe,
            fingers,
            dx,
            dy,
            scale: 1.0,
        }
    }

    #[test]
    fn swipes_and_pinches_pick_their_command() {
        assert_eq!(command_for(&swipe(3, -200.0, 10.0), None), Some(SPACE_NEXT));
        assert_eq!(
            command_for(&swipe(4, 200.0, 10.0), None),
            Some(SPACE_PREVIOUS)
        );
        assert_eq!(command_for(&swipe(3, 30.0, 0.0), None), None);
        assert_eq!(command_for(&swipe(2, -200.0, 0.0), None), None);
        assert_eq!(
            command_for(&swipe(3, 0.0, -150.0), None),
            Some(PANORAMA_TOGGLE)
        );
        assert_eq!(
            command_for(&swipe(3, 0.0, -150.0), Some(Mode::Panorama)),
            None
        );
        assert_eq!(command_for(&swipe(3, 0.0, 150.0), None), Some(PANORAMA_APP));
        assert_eq!(
            command_for(&swipe(3, 0.0, 150.0), Some(Mode::Panorama)),
            Some(PANORAMA_TOGGLE)
        );
        assert_eq!(
            command_for(&swipe(3, 0.0, 150.0), Some(Mode::App)),
            Some(PANORAMA_APP)
        );
        let pinch = |scale| Gesture {
            kind: GestureKind::Pinch,
            fingers: 4,
            dx: 0.0,
            dy: 0.0,
            scale,
        };
        assert_eq!(command_for(&pinch(1.6), None), Some(DESKTOP_SHOW));
        assert_eq!(command_for(&pinch(0.5), None), Some(LAUNCHER_TOGGLE));
        assert_eq!(command_for(&pinch(1.1), None), None);
    }
}
