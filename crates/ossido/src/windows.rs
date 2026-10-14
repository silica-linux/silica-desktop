//! Finestre ridotte e a schermo intero.

use std::cell::Cell;

use smithay::desktop::Window;
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::utils::{Logical, Point, Rectangle};

use crate::state::State;

/// Dove era la finestra prima dello schermo intero, nei suoi dati: la geometria e la cornice
/// (con la barra), per tornare e per l'animazione dell'uscita.
#[derive(Default)]
struct BeforeFullscreen(
    Cell<Option<Rectangle<i32, Logical>>>,
    Cell<Option<Rectangle<f64, Logical>>>,
);

pub fn is_fullscreen(window: &Window) -> bool {
    window.toplevel().is_some_and(|toplevel| {
        toplevel
            .current_state()
            .states
            .contains(xdg_toplevel::State::Fullscreen)
    })
}

/// Una disposizione del popover del pulsante verde (windows.md, W-08): una metà dell'area
/// utile o tutta. Margini fra le finestre: nessuno finché W-08 non li misura.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tile {
    Left,
    Right,
    Top,
    Bottom,
    Fill,
}

/// Le cornici di `count` finestre disposte nell'area (la prima è quella con il focus): 2 a
/// metà, 3 con la prima a sinistra e due impilate a destra, 4 nei quarti.
pub fn arrangement(area: Rectangle<i32, Logical>, count: usize) -> Vec<Rectangle<i32, Logical>> {
    let (x, y) = (area.loc.x, area.loc.y);
    let (w, h) = (area.size.w, area.size.h);
    let (hw, hh) = (w / 2, h / 2);
    let rect = |x: i32, y: i32, w: i32, h: i32| Rectangle::new((x, y).into(), (w, h).into());
    match count {
        0 => Vec::new(),
        1 => vec![area],
        2 => vec![rect(x, y, hw, h), rect(x + hw, y, w - hw, h)],
        3 => vec![
            rect(x, y, hw, h),
            rect(x + hw, y, w - hw, hh),
            rect(x + hw, y + hh, w - hw, h - hh),
        ],
        _ => vec![
            rect(x, y, hw, hh),
            rect(x + hw, y, w - hw, hh),
            rect(x, y + hh, hw, h - hh),
            rect(x + hw, y + hh, w - hw, h - hh),
        ],
    }
}

impl State {
    /// Porta la finestra in una metà dell'area utile, o in tutta (fuori dallo schermo intero).
    pub fn tile_window(&mut self, window: &Window, tile: Tile) -> Result<(), String> {
        let area = self.usable_area();
        let (x, y) = (area.loc.x, area.loc.y);
        let (w, h) = (area.size.w, area.size.h);
        let (hw, hh) = (w / 2, h / 2);
        let frame = match tile {
            Tile::Left => Rectangle::new((x, y).into(), (hw, h).into()),
            Tile::Right => Rectangle::new((x + hw, y).into(), (w - hw, h).into()),
            Tile::Top => Rectangle::new((x, y).into(), (w, hh).into()),
            Tile::Bottom => Rectangle::new((x, y + hh).into(), (w, h - hh).into()),
            Tile::Fill => area,
        };
        self.place_frame(window, frame)
    }

    /// Dispone la finestra e le `count − 1` usate più di recente dello spazio (dall'alto della
    /// pila) secondo [`arrangement`].
    pub fn arrange_windows(&mut self, first: &Window, count: usize) -> Result<(), String> {
        let mut windows = vec![first.clone()];
        windows.extend(
            self.space
                .elements()
                .rev()
                .filter(|w| *w != first)
                .take(count.saturating_sub(1))
                .cloned(),
        );
        let frames = arrangement(self.usable_area(), windows.len());
        for (window, frame) in windows.iter().zip(frames) {
            self.place_frame(window, frame)?;
        }
        self.focus_window(Some(first.clone()));
        Ok(())
    }

    /// Mette la cornice della finestra (barra compresa) in `frame`: posizione e dimensione del
    /// contenuto chiesta al client.
    fn place_frame(
        &mut self,
        window: &Window,
        frame: Rectangle<i32, Logical>,
    ) -> Result<(), String> {
        if is_fullscreen(window) {
            self.toggle_fullscreen(window)?;
        }
        let toplevel = window.toplevel().ok_or("finestra senza toplevel")?;
        let bar = crate::decorations::titlebar_height(window);
        let size = (frame.size.w, frame.size.h.saturating_sub(bar).max(1));
        toplevel.with_pending_state(|state| {
            state.size = Some(size.into());
        });
        toplevel.send_pending_configure();
        self.space
            .map_element(window.clone(), frame.loc + Point::from((0, bar)), false);
        Ok(())
    }

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
        let output = self
            .space
            .output_geometry(&self.output)
            .ok_or("output senza geometria")?;
        if is_fullscreen(window) {
            let previous = before.0.take();
            let frame = before.1.take();
            toplevel.with_pending_state(|state| {
                state.states.unset(xdg_toplevel::State::Fullscreen);
                state.size = previous.map(|rect| rect.size);
            });
            toplevel.send_pending_configure();
            if let Some(previous) = previous {
                self.space.map_element(window.clone(), previous.loc, true);
            }
            if let Some(frame) = frame {
                self.start_morph(window, output.to_f64(), frame, false);
            }
        } else {
            let current = self
                .space
                .element_geometry(window)
                .ok_or("finestra non sullo schermo")?;
            let frame = crate::decorations::frame_of(window, current).to_f64();
            before.0.set(Some(current));
            before.1.set(Some(frame));
            self.start_morph(window, frame, output.to_f64(), true);
            toplevel.with_pending_state(|state| {
                state.states.set(xdg_toplevel::State::Fullscreen);
                state.size = Some(output.size);
            });
            toplevel.send_pending_configure();
            self.space.map_element(window.clone(), output.loc, true);
        }
        Ok(())
    }

    /// L'animazione dello schermo intero: la cornice va da `from` a `to` (spenta con le
    /// animazioni a 0).
    fn start_morph(
        &mut self,
        window: &Window,
        from: Rectangle<f64, Logical>,
        to: Rectangle<f64, Logical>,
        entering: bool,
    ) {
        if self.animation_speed <= 0.0 {
            return;
        }
        window
            .user_data()
            .insert_if_missing(crate::render::FullscreenMorph::default);
        if let Some(morph) = window.user_data().get::<crate::render::FullscreenMorph>() {
            morph.0.set(Some(crate::render::Morph {
                from,
                to,
                start: std::time::Instant::now(),
                entering,
            }));
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arrangements_split_the_area() {
        let area = Rectangle::new((0, 30).into(), (1600, 900).into());
        let two = arrangement(area, 2);
        assert_eq!(two[0], Rectangle::new((0, 30).into(), (800, 900).into()));
        assert_eq!(two[1], Rectangle::new((800, 30).into(), (800, 900).into()));
        let three = arrangement(area, 3);
        assert_eq!(
            three[1],
            Rectangle::new((800, 30).into(), (800, 450).into())
        );
        assert_eq!(
            three[2],
            Rectangle::new((800, 480).into(), (800, 450).into())
        );
        let four = arrangement(area, 4);
        assert_eq!(
            four[3],
            Rectangle::new((800, 480).into(), (800, 450).into())
        );
        assert!(arrangement(area, 0).is_empty());
    }
}
