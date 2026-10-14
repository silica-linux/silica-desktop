//! Transient UI: the menus and panels of the shell (`silica_shell_manager_v1.set_transient`)
//! and the app popups with a grab (`xdg_popup.grab`).
//!
//! While one is open the other surfaces keep the pointer (hover, cursor shapes): the shell no
//! longer covers the screen with its input region. A button press outside every open
//! transient ends them; per transient, the press is then consumed or delivered to what is
//! under the pointer (#149). Scrolling outside is delivered or dropped, without ending them.
//! The defaults are provisional until the reference's INPUT-01 measurement.

use silica_protocols::shell::server::silica_shell_manager_v1::{Outside, SilicaShellManagerV1};
use smithay::desktop::{PopupKind, PopupManager, find_popup_root_surface};
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{IsAlive, Logical, Point, Rectangle};
use smithay::wayland::compositor::get_parent;

use crate::state::State;

/// What an input outside an open transient does after closing it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutsidePolicy {
    /// It closes the transient and nothing else.
    Consume,
    /// It closes the transient and also reaches what is below.
    PassThrough,
}

impl OutsidePolicy {
    pub fn from_protocol(value: Outside) -> Self {
        match value {
            Outside::PassThrough => Self::PassThrough,
            _ => Self::Consume,
        }
    }
}

/// Who opened the transient.
#[derive(Debug, Clone)]
pub enum Owner {
    /// A shell surface (menu bar, Dock, Faro…), told with `dismissed`.
    Shell {
        surface: WlSurface,
        manager: SilicaShellManagerV1,
    },
    /// An app popup with a grab, closed with `popup_done`.
    Popup { popup: PopupKind, root: WlSurface },
}

#[derive(Debug, Clone)]
pub struct Transient {
    pub owner: Owner,
    /// Areas that count as inside even under another surface (global, logical).
    pub region: Vec<Rectangle<i32, Logical>>,
    pub press: OutsidePolicy,
    pub scroll: OutsidePolicy,
}

impl Transient {
    /// The surface the transient belongs to (for replacing and clearing).
    pub fn surface(&self) -> &WlSurface {
        match &self.owner {
            Owner::Shell { surface, .. } => surface,
            Owner::Popup { root, .. } => root,
        }
    }
}

/// The outcome of a press with transients open: which ones end, and whether the press stops
/// there. `inside[i]` says whether the press is inside transient `i`.
pub fn press_outcome(inside: &[bool], press: &[OutsidePolicy]) -> (Vec<usize>, bool) {
    let dismissed: Vec<usize> = inside
        .iter()
        .enumerate()
        .filter(|(_, inside)| !**inside)
        .map(|(index, _)| index)
        .collect();
    let consume = dismissed
        .iter()
        .any(|index| press.get(*index) == Some(&OutsidePolicy::Consume));
    (dismissed, consume)
}

/// Whether a scroll outside the transients is dropped: when it is outside all of them and
/// one of those asks to consume it.
pub fn scroll_dropped(inside: &[bool], scroll: &[OutsidePolicy]) -> bool {
    !inside.iter().any(|inside| *inside)
        && inside
            .iter()
            .zip(scroll)
            .any(|(_, policy)| *policy == OutsidePolicy::Consume)
}

/// The top surface of a tree: up through subsurfaces and popups.
fn root_of(popups: &PopupManager, surface: &WlSurface) -> WlSurface {
    let mut current = surface.clone();
    for _ in 0..64 {
        if let Some(parent) = get_parent(&current) {
            current = parent;
            continue;
        }
        match popups.find_popup(&current) {
            Some(popup) => match find_popup_root_surface(&popup) {
                Ok(root) if root != current => current = root,
                _ => break,
            },
            None => break,
        }
    }
    current
}

impl State {
    /// Whether a point with this surface under it is inside each open transient.
    fn inside_transients(
        &self,
        under: Option<&WlSurface>,
        location: Point<f64, Logical>,
    ) -> Vec<bool> {
        let under_root = under.map(|surface| root_of(&self.popups, surface));
        self.transients
            .iter()
            .map(|transient| {
                let in_region = transient
                    .region
                    .iter()
                    .any(|rect| rect.to_f64().contains(location));
                in_region
                    || match &transient.owner {
                        Owner::Shell { surface, .. } => under_root.as_ref() == Some(surface),
                        // Like the popup grabs of the toolkits: the popup's own client is inside.
                        Owner::Popup { popup, .. } => under.is_some_and(|under| {
                            under.client().is_some()
                                && under.client() == popup.wl_surface().client()
                        }),
                    }
            })
            .collect()
    }

    /// A press: ends the transients it is outside of. True if the press stops there.
    pub fn press_on_transients(&mut self, location: Point<f64, Logical>) -> bool {
        self.transients.retain(|transient| match &transient.owner {
            Owner::Shell { surface, manager } => surface.is_alive() && manager.is_alive(),
            Owner::Popup { popup, .. } => popup.alive(),
        });
        if self.transients.is_empty() {
            return false;
        }
        let under = self.surface_under(location).map(|(surface, _)| surface);
        let inside = self.inside_transients(under.as_ref(), location);
        let press: Vec<OutsidePolicy> = self.transients.iter().map(|t| t.press).collect();
        let (dismissed, consume) = press_outcome(&inside, &press);
        let mut ended = Vec::new();
        for index in dismissed.into_iter().rev() {
            if index < self.transients.len() {
                ended.push(self.transients.remove(index));
            }
        }
        for transient in ended {
            self.end_transient(transient);
        }
        consume
    }

    /// A scroll: true if it must not reach the surface under the pointer.
    pub fn scroll_on_transients(&self, location: Point<f64, Logical>) -> bool {
        if self.transients.is_empty() {
            return false;
        }
        let under = self.surface_under(location).map(|(surface, _)| surface);
        let inside = self.inside_transients(under.as_ref(), location);
        let scroll: Vec<OutsidePolicy> = self.transients.iter().map(|t| t.scroll).collect();
        scroll_dropped(&inside, &scroll)
    }

    fn end_transient(&mut self, transient: Transient) {
        match transient.owner {
            Owner::Shell { surface, manager } => {
                if manager.version() >= 6 {
                    manager.dismissed(&surface);
                }
            }
            Owner::Popup { popup, root } => {
                if let Err(err) = PopupManager::dismiss_popup(&root, &popup) {
                    tracing::warn!("popup dismiss: {err}");
                }
                // The keyboard goes back to the popup's window.
                if let Some(keyboard) = self.seat.get_keyboard()
                    && keyboard
                        .current_focus()
                        .is_some_and(|focus| focus == *popup.wl_surface())
                {
                    keyboard.set_focus(
                        self,
                        Some(root),
                        smithay::utils::SERIAL_COUNTER.next_serial(),
                    );
                }
            }
        }
    }

    /// Where a layer surface (or its tree) sits on the screen, logical; the output origin if
    /// it is not a layer surface.
    pub fn surface_origin(&self, surface: &WlSurface) -> Point<i32, Logical> {
        let origin = self
            .space
            .output_geometry(&self.output)
            .map(|g| g.loc)
            .unwrap_or_default();
        let map = smithay::desktop::layer_map_for_output(&self.output);
        map.layer_for_surface(surface, smithay::desktop::WindowSurfaceType::ALL)
            .and_then(|layer| map.layer_geometry(layer))
            .map_or(origin, |geometry| geometry.loc + origin)
    }

    /// A shell surface opens (or replaces) its transient.
    pub fn set_shell_transient(
        &mut self,
        manager: SilicaShellManagerV1,
        surface: WlSurface,
        region: Vec<Rectangle<i32, Logical>>,
        press: OutsidePolicy,
        scroll: OutsidePolicy,
    ) {
        self.transients.retain(|t| t.surface() != &surface);
        self.transients.push(Transient {
            owner: Owner::Shell { surface, manager },
            region,
            press,
            scroll,
        });
    }

    /// The shell closed its transient itself.
    pub fn clear_shell_transient(&mut self, surface: &WlSurface) {
        self.transients
            .retain(|t| !matches!(&t.owner, Owner::Shell { surface: s, .. } if s == surface));
    }

    /// An app popup asks for a grab: it closes on a press outside its app, and has the
    /// keyboard while open. Menus consume that press (provisional, INPUT-01).
    pub fn grab_popup(&mut self, popup: PopupKind) {
        let Ok(root) = find_popup_root_surface(&popup) else {
            return;
        };
        let surface = popup.wl_surface().clone();
        self.transients.retain(|t| match &t.owner {
            Owner::Popup { root: r, .. } => r != &root,
            Owner::Shell { .. } => true,
        });
        self.transients.push(Transient {
            owner: Owner::Popup { popup, root },
            region: Vec::new(),
            press: OutsidePolicy::Consume,
            scroll: OutsidePolicy::PassThrough,
        });
        if let Some(keyboard) = self.seat.get_keyboard() {
            keyboard.set_focus(
                self,
                Some(surface),
                smithay::utils::SERIAL_COUNTER.next_serial(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{OutsidePolicy, press_outcome, scroll_dropped};
    use OutsidePolicy::{Consume, PassThrough};

    #[test]
    fn a_press_inside_keeps_every_transient() {
        assert_eq!(press_outcome(&[true], &[Consume]), (vec![], false));
    }

    #[test]
    fn a_press_outside_a_menu_closes_it_and_stops_there() {
        assert_eq!(press_outcome(&[false], &[Consume]), (vec![0], true));
    }

    #[test]
    fn a_press_outside_a_panel_closes_it_and_goes_through() {
        assert_eq!(press_outcome(&[false], &[PassThrough]), (vec![0], false));
    }

    #[test]
    fn with_two_open_the_one_left_decides() {
        // Inside the first (the press lands there), outside the second: only the second ends.
        assert_eq!(
            press_outcome(&[true, false], &[Consume, PassThrough]),
            (vec![1], false)
        );
        assert_eq!(
            press_outcome(&[false, false], &[PassThrough, Consume]),
            (vec![0, 1], true)
        );
    }

    #[test]
    fn scrolling_outside_follows_the_policy() {
        assert!(!scroll_dropped(&[true], &[Consume]));
        assert!(scroll_dropped(&[false], &[Consume]));
        assert!(!scroll_dropped(&[false], &[PassThrough]));
    }
}
