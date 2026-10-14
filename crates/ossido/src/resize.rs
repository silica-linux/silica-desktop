//! Ridimensionamento dai bordi della cornice (mustache-spec 1.1, "Zone di trascinamento e
//! bordi", MU-08): il bordo sinistro e quello alto prendono il puntatore da 2 pt fuori a 3 pt
//! dentro la cornice, il destro e il basso da 4 pt fuori a 3 dentro; vicino a un angolo, per
//! 16 pt lungo ciascun bordo, il ridimensionamento è diagonale. Vale anche per le finestre a
//! contenuto a tutta finestra (la cornice comprende la barra dell'app) e per le richieste
//! `xdg_toplevel.resize` dei client.

use smithay::desktop::Window;
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::utils::{Logical, Point, Rectangle, Size};
use smithay::wayland::compositor::with_states;
use smithay::wayland::shell::xdg::SurfaceCachedState;

use crate::state::State;

/// Fuori dalla cornice a sinistra e in alto.
pub const OUTSIDE_LEADING: f64 = 2.0;
/// Fuori dalla cornice a destra e in basso.
pub const OUTSIDE_TRAILING: f64 = 4.0;
/// Dentro la cornice, su tutti i lati.
pub const INSIDE: f64 = 3.0;
/// Lungo un bordo, dall'angolo: la zona diagonale.
pub const CORNER: f64 = 16.0;
/// Il contenuto non scende sotto questo lato se il client non chiede altro.
const MIN_CONTENT: i32 = 1;

/// I lati che il puntatore sposta.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Edges {
    pub left: bool,
    pub right: bool,
    pub top: bool,
    pub bottom: bool,
}

impl Edges {
    pub fn any(self) -> bool {
        self.left || self.right || self.top || self.bottom
    }

    /// I lati di `xdg_toplevel.resize`.
    pub fn from_xdg(edge: xdg_toplevel::ResizeEdge) -> Self {
        use xdg_toplevel::ResizeEdge as E;
        Self {
            left: matches!(edge, E::Left | E::TopLeft | E::BottomLeft),
            right: matches!(edge, E::Right | E::TopRight | E::BottomRight),
            top: matches!(edge, E::Top | E::TopLeft | E::TopRight),
            bottom: matches!(edge, E::Bottom | E::BottomLeft | E::BottomRight),
        }
    }
}

/// I lati sotto il punto `p` per la cornice `frame`; None fuori dalle zone.
pub fn edges_at(frame: Rectangle<i32, Logical>, p: Point<f64, Logical>) -> Option<Edges> {
    let left = f64::from(frame.loc.x);
    let top = f64::from(frame.loc.y);
    let right = left + f64::from(frame.size.w);
    let bottom = top + f64::from(frame.size.h);
    // Dentro il rettangolo allargato delle zone.
    if p.x < left - OUTSIDE_LEADING
        || p.x >= right + OUTSIDE_TRAILING
        || p.y < top - OUTSIDE_LEADING
        || p.y >= bottom + OUTSIDE_TRAILING
    {
        return None;
    }
    let mut edges = Edges {
        left: p.x < left + INSIDE,
        right: p.x >= right - INSIDE,
        top: p.y < top + INSIDE,
        bottom: p.y >= bottom - INSIDE,
    };
    // Una finestra più stretta delle due zone: vince il lato più vicino.
    if edges.left && edges.right {
        let to_left = (p.x - left).abs();
        let to_right = (right - p.x).abs();
        edges.left = to_left <= to_right;
        edges.right = !edges.left;
    }
    if edges.top && edges.bottom {
        let to_top = (p.y - top).abs();
        let to_bottom = (bottom - p.y).abs();
        edges.top = to_top <= to_bottom;
        edges.bottom = !edges.top;
    }
    // Vicino a un angolo il bordo diventa diagonale.
    if (edges.left || edges.right) && !(edges.top || edges.bottom) {
        edges.top = p.y < top + CORNER;
        edges.bottom = !edges.top && p.y >= bottom - CORNER;
    } else if (edges.top || edges.bottom) && !(edges.left || edges.right) {
        edges.left = p.x < left + CORNER;
        edges.right = !edges.left && p.x >= right - CORNER;
    }
    edges.any().then_some(edges)
}

/// La nuova cornice dopo uno spostamento `delta` del puntatore, entro i limiti del contenuto
/// (`min` e `max`, 0 senza limite) e con la barra `bar` sopra; il lato opposto resta fermo.
pub fn resized(
    frame: Rectangle<i32, Logical>,
    edges: Edges,
    delta: Point<f64, Logical>,
    bar: i32,
    min: Size<i32, Logical>,
    max: Size<i32, Logical>,
) -> Rectangle<i32, Logical> {
    let dx = delta.x.round() as i32;
    let dy = delta.y.round() as i32;
    let clamp = |value: i32, low: i32, high: i32| {
        let value = value.max(low);
        if high > 0 {
            value.min(high.max(low))
        } else {
            value
        }
    };
    let mut w = frame.size.w;
    let mut h = frame.size.h;
    if edges.left {
        w = w.saturating_sub(dx);
    } else if edges.right {
        w = w.saturating_add(dx);
    }
    if edges.top {
        h = h.saturating_sub(dy);
    } else if edges.bottom {
        h = h.saturating_add(dy);
    }
    let min_w = min.w.max(MIN_CONTENT);
    let min_h = min.h.max(MIN_CONTENT).saturating_add(bar);
    let max_h = if max.h > 0 {
        max.h.saturating_add(bar)
    } else {
        0
    };
    w = clamp(w, min_w, max.w);
    h = clamp(h, min_h, max_h);
    let right = frame.loc.x.saturating_add(frame.size.w);
    let bottom = frame.loc.y.saturating_add(frame.size.h);
    let x = if edges.left {
        right.saturating_sub(w)
    } else {
        frame.loc.x
    };
    let y = if edges.top {
        bottom.saturating_sub(h)
    } else {
        frame.loc.y
    };
    Rectangle::new((x, y).into(), (w, h).into())
}

/// Un ridimensionamento in corso: la finestra, i lati, dove è iniziato e la cornice di allora.
#[derive(Debug, Clone)]
pub struct Resize {
    pub window: Window,
    pub edges: Edges,
    pub start: Point<f64, Logical>,
    pub frame: Rectangle<i32, Logical>,
}

impl State {
    /// La finestra il cui bordo è sotto il punto, dall'alto: una finestra che copre il punto
    /// fuori dalle sue zone ferma la ricerca (non si ridimensiona quella sotto).
    pub fn resize_hit(&self, location: Point<f64, Logical>) -> Option<(Window, Edges)> {
        for window in self.space.elements().rev() {
            if crate::windows::is_fullscreen(window) {
                continue;
            }
            let Some(frame) = self.window_frame(window) else {
                continue;
            };
            if let Some(edges) = edges_at(frame, location) {
                return Some((window.clone(), edges));
            }
            if frame.to_f64().contains(location) {
                return None;
            }
        }
        None
    }

    /// Comincia a ridimensionare `window` dai lati `edges`, dal punto `start`.
    pub fn start_resize(&mut self, window: Window, edges: Edges, start: Point<f64, Logical>) {
        let Some(frame) = self.window_frame(&window) else {
            return;
        };
        if let Some(toplevel) = window.toplevel() {
            toplevel.with_pending_state(|state| {
                state.states.set(xdg_toplevel::State::Resizing);
            });
            toplevel.send_pending_configure();
        }
        self.resizing = Some(Resize {
            window,
            edges,
            start,
            frame,
        });
    }

    /// Il puntatore si è mosso durante un ridimensionamento: nuova dimensione al client e, per
    /// i lati sinistro e alto, nuova posizione.
    pub fn resize_to(&mut self, location: Point<f64, Logical>) {
        let Some(resize) = self.resizing.clone() else {
            return;
        };
        let Some(toplevel) = resize.window.toplevel() else {
            return;
        };
        let (min, max) = with_states(toplevel.wl_surface(), |states| {
            let mut cached = states.cached_state.get::<SurfaceCachedState>();
            let current = cached.current();
            (current.min_size, current.max_size)
        });
        let bar = crate::decorations::titlebar_height(&resize.window);
        let frame = resized(
            resize.frame,
            resize.edges,
            location - resize.start,
            bar,
            min,
            max,
        );
        let content = Size::from((frame.size.w, frame.size.h.saturating_sub(bar).max(1)));
        toplevel.with_pending_state(|state| {
            state.size = Some(content);
        });
        toplevel.send_pending_configure();
        if resize.edges.left || resize.edges.top {
            self.space.map_element(
                resize.window.clone(),
                frame.loc + Point::from((0, bar)),
                false,
            );
        }
    }

    /// Fine del ridimensionamento (rilascio del pulsante).
    pub fn end_resize(&mut self) {
        let Some(resize) = self.resizing.take() else {
            return;
        };
        if let Some(toplevel) = resize.window.toplevel() {
            toplevel.with_pending_state(|state| {
                state.states.unset(xdg_toplevel::State::Resizing);
            });
            toplevel.send_pending_configure();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame() -> Rectangle<i32, Logical> {
        Rectangle::new((200, 150).into(), (1000, 700).into())
    }

    fn at(x: f64, y: f64) -> Option<Edges> {
        edges_at(frame(), Point::from((x, y)))
    }

    const L: Edges = Edges {
        left: true,
        right: false,
        top: false,
        bottom: false,
    };

    #[test]
    fn zone_misurate_sui_quattro_lati() {
        // Sinistra: x 198–202 (bordo a 200).
        assert_eq!(at(197.9, 400.0), None);
        assert_eq!(at(198.0, 400.0), Some(L));
        assert_eq!(at(202.9, 400.0), Some(L));
        assert_eq!(at(203.0, 400.0), None);
        // Destra: bordo a 1200, 3 dentro e 4 fuori.
        assert_eq!(at(1196.9, 400.0), None);
        assert!(at(1197.0, 400.0).is_some_and(|e| e.right && !e.top && !e.bottom));
        assert!(at(1203.9, 400.0).is_some_and(|e| e.right));
        assert_eq!(at(1204.0, 400.0), None);
        // Alto e basso.
        assert!(at(600.0, 148.0).is_some_and(|e| e.top && !e.left && !e.right));
        assert_eq!(at(600.0, 147.9), None);
        assert_eq!(at(600.0, 153.0), None);
        assert!(at(600.0, 847.0).is_some_and(|e| e.bottom));
        assert!(at(600.0, 853.9).is_some_and(|e| e.bottom));
        assert_eq!(at(600.0, 854.0), None);
    }

    #[test]
    fn angoli_per_16_pt() {
        let e = at(199.0, 165.0).unwrap();
        assert!(e.left && e.top);
        let e = at(199.0, 166.0).unwrap();
        assert!(e.left && !e.top);
        let e = at(215.0, 149.0).unwrap();
        assert!(e.left && e.top);
        let e = at(1201.0, 840.0).unwrap();
        assert!(e.right && e.bottom);
        let e = at(1190.0, 851.0).unwrap();
        assert!(e.right && e.bottom);
    }

    #[test]
    fn la_cornice_segue_e_rispetta_i_limiti() {
        let none = Size::from((0, 0));
        let both = Edges {
            left: true,
            top: true,
            ..Edges::default()
        };
        let r = resized(frame(), both, Point::from((50.0, 30.0)), 52, none, none);
        assert_eq!(r, Rectangle::new((250, 180).into(), (950, 670).into()));
        // Il minimo del client (contenuto 600 × 300): il lato destro e il basso restano.
        let min = Size::from((600, 300));
        let r = resized(frame(), both, Point::from((900.0, 900.0)), 52, min, none);
        assert_eq!(r.size, Size::from((600, 352)));
        assert_eq!((r.loc.x + r.size.w, r.loc.y + r.size.h), (1200, 850));
        let right = Edges {
            right: true,
            ..Edges::default()
        };
        let max = Size::from((1100, 0));
        let r = resized(frame(), right, Point::from((500.0, 0.0)), 0, none, max);
        assert_eq!(r, Rectangle::new((200, 150).into(), (1100, 700).into()));
    }
}
