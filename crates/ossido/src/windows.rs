//! Minimized and full-screen windows.

use std::cell::Cell;

use smithay::desktop::Window;
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point, Rectangle};

use crate::state::State;

/// Where the window was before full screen, in its data: the geometry and the frame (with
/// the bar), to go back and for the exit animation.
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

/// An arrangement of the green button's popover (windows.md, W-08): a half of the usable area
/// or all of it. Margins between the windows: none until W-08 measures them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tile {
    Left,
    Right,
    Top,
    Bottom,
    Fill,
}

/// The frames of `count` windows arranged in the area (the first is the focused one): 2 in
/// halves, 3 with the first on the left and two stacked on the right, 4 in quarters.
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
    /// Moves the window to a half of the usable area, or to all of it (outside full screen).
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

    /// Arranges the window and the `count − 1` most recently used ones of the space (from the
    /// top of the stack) according to [`arrangement`].
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

    /// Puts the window frame (bar included) in `frame`: position, and content size requested
    /// from the client.
    fn place_frame(
        &mut self,
        window: &Window,
        frame: Rectangle<i32, Logical>,
    ) -> Result<(), String> {
        if is_fullscreen(window) {
            self.toggle_fullscreen(window)?;
        }
        let toplevel = window.toplevel().ok_or("window without a toplevel")?;
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

    /// Where the thumbnail of a minimized window ends up, in output points: the cell before
    /// the Trash in the Dock (the widest glass panel of the `dock` layer). From the minimize
    /// recording (`windows.md`): the Dock grows by one cell (816–1743 → 784–1775) and the
    /// thumbnail stops centered at x 1672, that is 71 pt to the left of the previous right
    /// edge; vertically at the center of the Dock. Thumbnail 57 wide and at most 47 tall
    /// (`dock.md`, D-08). Without a Dock: at the bottom center of the screen.
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

    /// Removes the window from the screen and gives the focus to the one below.
    pub fn minimize_window(&mut self, window: &Window) -> Result<(), String> {
        let location = self
            .space
            .element_location(window)
            .ok_or("window not on screen")?;
        // The thumbnail for the Dock and the genie, while the window is still on screen.
        self.capture_thumbnail(window);
        self.start_genie(window, false);
        self.space.unmap_elem(window);
        self.minimized.push((window.clone(), location));
        self.announce_minimized(window, true);
        let next = self.space.elements().last().cloned();
        self.focus_window(next);
        self.desktop_owner_fallback(None);
        Ok(())
    }

    /// Brings a minimized window back where it was. False if it was not minimized.
    pub fn restore_window(&mut self, window: &Window) -> bool {
        let Some(index) = self.minimized.iter().position(|(w, _)| w == window) else {
            return false;
        };
        let (window, location) = self.minimized.remove(index);
        self.announce_minimized(&window, false);
        self.space.map_element(window.clone(), location, true);
        // It comes out of the Dock as a genie; until the genie ends the window stays hidden.
        self.start_genie(&window, true);
        true
    }

    /// The app closed its last window but keeps running (#142): the window leaves the screen,
    /// the shell's list and the focus, fading out like a closed window, and waits in `hidden`
    /// with its position. A minimized window is hidden too, from the Dock.
    pub fn hide_window(&mut self, surface: &WlSurface) {
        let on_screen = self
            .space
            .elements()
            .find(|w| w.toplevel().is_some_and(|t| t.wl_surface() == surface))
            .cloned();
        let entry = if let Some(window) = on_screen {
            let location = self.space.element_location(&window).unwrap_or_default();
            self.start_closing(&window);
            self.space.unmap_elem(&window);
            (window, location)
        } else if let Some(index) = self
            .minimized
            .iter()
            .position(|(w, _)| w.toplevel().is_some_and(|t| t.wl_surface() == surface))
        {
            let (window, location) = self.minimized.remove(index);
            self.announce_minimized(&window, false);
            (window, location)
        } else {
            return;
        };
        self.withdraw_toplevel(&entry.0);
        let app = crate::panorama::window_app(&entry.0);
        self.hidden.push(entry);
        // As on the reference (APP-LC) the app stays frontmost with no window: no other
        // window takes the focus, and the menu bar keeps showing the app.
        self.focus_window(None);
        self.set_windowless_front(app);
    }

    /// The windowless frontmost app (#142), announced to the shell as `app.frontmost`.
    pub fn set_windowless_front(&mut self, app: Option<String>) {
        if self.windowless_front == app {
            return;
        }
        self.windowless_front = app;
        let value = self.windowless_front.clone().unwrap_or_default();
        self.broadcast_to_shell("app.frontmost".to_owned(), value);
    }

    /// The app shows its hidden window again: back where it was, in the shell's list, focused.
    pub fn show_window(&mut self, surface: &WlSurface) {
        let Some(index) = self
            .hidden
            .iter()
            .position(|(w, _)| w.toplevel().is_some_and(|t| t.wl_surface() == surface))
        else {
            return;
        };
        let (window, location) = self.hidden.remove(index);
        self.space.map_element(window.clone(), location, true);
        self.publish_toplevel(&window);
        self.focus_window(Some(window));
    }

    /// Full screen on the output, or back to the previous geometry.
    pub fn toggle_fullscreen(&mut self, window: &Window) -> Result<(), String> {
        let toplevel = window.toplevel().ok_or("window without a toplevel")?;
        window
            .user_data()
            .insert_if_missing(BeforeFullscreen::default);
        let before = window
            .user_data()
            .get::<BeforeFullscreen>()
            .ok_or("window state missing")?;
        let output = self
            .space
            .output_geometry(&self.output)
            .ok_or("output without geometry")?;
        if is_fullscreen(window) {
            // Leaving full screen: back to the space before the full-screen one, which goes
            // once the scroll has left it.
            let owned = self.owned_space(window);
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
            if let Some(index) = owned {
                let target = index.saturating_sub(1);
                self.move_window_to_space(window, target)?;
                self.workspaces.pending_removal = Some(window.clone());
                if !self.switch_space(target, std::time::Instant::now()) {
                    self.settle_transition();
                }
                self.focus_window(Some(window.clone()));
            }
        } else {
            let current = self
                .space
                .element_geometry(window)
                .ok_or("window not on screen")?;
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
            // A full-screen window is a space of its own, right after the one it came from.
            let from = self.window_space(window).unwrap_or(self.workspaces.active);
            match self.insert_space(from.saturating_add(1)) {
                Ok(index) => {
                    if let Some(slot) = self.workspaces.owners.get_mut(index) {
                        *slot = Some(window.clone());
                    }
                    self.move_window_to_space(window, index)?;
                    if !self.switch_space(index, std::time::Instant::now()) {
                        self.settle_transition();
                    }
                    self.focus_window(Some(window.clone()));
                }
                // No room for another space: full screen in place.
                Err(err) => tracing::info!("full screen: {err}"),
            }
        }
        Ok(())
    }

    /// The full-screen animation: the frame goes from `from` to `to` (off with animations
    /// at 0).
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

    /// A minimized window with that id.
    pub fn minimized_window(&self, id: u64) -> Option<Window> {
        self.minimized
            .iter()
            .find(|(window, _)| Self::window_id(window) == Some(id))
            .map(|(window, _)| window.clone())
    }
}

/// Saved position of a minimized window.
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
