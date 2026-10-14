//! Input: keyboard and pointer to the windows.

use smithay::backend::input::{
    AbsolutePositionEvent, Axis, AxisSource, ButtonState, Event, GestureBeginEvent,
    GestureEndEvent, GesturePinchUpdateEvent, GestureSwipeUpdateEvent, InputBackend, InputEvent,
    KeyState, KeyboardKeyEvent, PointerAxisEvent, PointerButtonEvent, PointerMotionEvent,
};
use smithay::input::keyboard::{FilterResult, Keycode, xkb};
use smithay::input::pointer::{AxisFrame, ButtonEvent, MotionEvent};
use smithay::utils::{Logical, Point, Rectangle, SERIAL_COUNTER};
use smithay::wayland::shell::wlr_layer::Layer;
use tracing::warn;

use ossido_render::titlebar::{self, Part};

use crate::decorations::{self, Hit};
use crate::state::State;

impl State {
    /// Moves the pointer to a point of the desktop (logical pixels).
    pub fn pointer_move_to(&mut self, location: Point<f64, Logical>, time: u32) {
        let Some(pointer) = self.seat.get_pointer() else {
            return;
        };
        if self.resizing.is_some() {
            self.resize_to(location);
        }
        // Dragging by the bar: the frame follows the grab point.
        if let Some((window, grab)) = self.moving.clone() {
            let bar = decorations::titlebar_height(&window);
            let frame = (location - grab).to_i32_round::<i32>();
            self.space
                .map_element(window, frame + Point::from((0, bar)), false);
        }
        let over_layer = self
            .layer_under(&[Layer::Overlay, Layer::Top], location)
            .is_some();
        // In Panorama the windows do not receive the pointer (they are drawn elsewhere).
        let in_panorama = self.panorama.is_some() && !self.over_dock_layer(location);
        if self.panorama.is_some() {
            self.panorama_pointer(location, std::time::Instant::now());
        }
        let hit = if over_layer || in_panorama {
            None
        } else {
            self.window_hit(location)
        };
        let hover = match &hit {
            Some(Hit::Titlebar(window, _, local)) if titlebar::over_buttons(*local) => {
                Some(window.clone())
            }
            _ => None,
        };
        // Symbols: they appear on entering, they disappear shortly after leaving.
        let now = std::time::Instant::now();
        let same = matches!((&hover, &self.buttons_hover),
            (Some(w), Some(h)) if *w == h.window && h.left.is_none());
        if !same {
            match &hover {
                Some(window) => {
                    self.buttons_hover = Some(crate::state::ButtonsHover {
                        window: window.clone(),
                        entered: now,
                        left: None,
                    });
                }
                None => {
                    if let Some(h) = self.buttons_hover.as_mut()
                        && h.left.is_none()
                    {
                        h.left = Some(now);
                    }
                }
            }
        }
        self.titlebar_press_inside = match (&self.titlebar_press, &hit) {
            (Some((pressed, part)), Some(Hit::Titlebar(window, over, _))) => {
                pressed == window && part == over
            }
            _ => false,
        };
        self.titlebar_hover = hover;
        // On the bar the pointer is not on any client surface.
        let under = match hit {
            Some(Hit::Titlebar(..)) => None,
            _ if in_panorama => None,
            _ => self.surface_under(location),
        };
        pointer.motion(
            self,
            under,
            &MotionEvent {
                location,
                serial: SERIAL_COUNTER.next_serial(),
                time,
            },
        );
        pointer.frame(self);
    }

    /// Presses or releases a button; the press gives the focus to the window below.
    pub fn pointer_button(&mut self, button: u32, state: ButtonState, time: u32) {
        let Some(pointer) = self.seat.get_pointer() else {
            return;
        };
        let serial = SERIAL_COUNTER.next_serial();
        let location = pointer.current_location();
        // Panorama takes the clicks, except those on the Dock and on the panels.
        if state == ButtonState::Pressed && self.panorama_open() && !self.over_dock_layer(location)
        {
            self.panorama_press = true;
            self.panorama_press_at(location, std::time::Instant::now());
            return;
        }
        if state == ButtonState::Released && self.panorama_press {
            self.panorama_press = false;
            self.panorama_release(location, std::time::Instant::now());
            return;
        }
        // An open menu or panel (#149): a press outside it ends it; per transient the press
        // stops there, and then its release too.
        if state == ButtonState::Pressed
            && !pointer.is_grabbed()
            && self.press_on_transients(location)
        {
            self.transient_press = true;
            return;
        }
        if state == ButtonState::Released && self.transient_press {
            self.transient_press = false;
            return;
        }
        if state == ButtonState::Pressed && !pointer.is_grabbed() {
            match self.layer_under(&[Layer::Overlay, Layer::Top], location) {
                // A click on a layer does not touch the windows; the focus only if it asks for it.
                Some((layer, _)) => {
                    if layer.can_receive_keyboard_focus() {
                        self.focus_layer(&layer);
                    }
                }
                // The frame edges resize, even just outside (MU-08).
                None if !self.panorama_open()
                    && let Some((window, edges)) = self.resize_hit(location) =>
                {
                    self.focus_window(Some(window.clone()));
                    self.start_resize(window, edges, location);
                }
                None => match self.window_hit(location) {
                    Some(Hit::Titlebar(window, part, local)) => {
                        self.focus_window(Some(window.clone()));
                        if part == Part::Bar {
                            self.moving = Some((window, local));
                        } else {
                            self.titlebar_press = Some((window.clone(), part));
                            self.titlebar_press_inside = true;
                            if part == Part::Zoom
                                && !crate::render::buttons_disabled(&window).zoom()
                            {
                                self.arm_window_menu(&window);
                            }
                        }
                    }
                    Some(Hit::Content(window)) => self.focus_window(Some(window)),
                    None => self.focus_window(None),
                },
            }
        }
        if state == ButtonState::Pressed {
            self.press_serial = Some(serial);
        }
        if state == ButtonState::Released {
            self.press_serial = None;
            self.moving = None;
            self.end_resize();
            if let Some((window, part)) = self.titlebar_press.take()
                && let Some(Hit::Titlebar(over, over_part, _)) = self.window_hit(location)
                && over == window
                && over_part == part
            {
                self.titlebar_action(&window, part);
            }
        }
        pointer.button(
            self,
            &ButtonEvent {
                button,
                state,
                serial,
                time,
            },
        );
        pointer.frame(self);
    }

    /// A key, real or synthetic (`input.key`): first the catalog shortcuts, which run their
    /// command and do not reach the app; the rest goes to the focused surface.
    pub fn handle_key(&mut self, code: Keycode, state: KeyState, time: u32) {
        let code = self.logo_key.remap(code);
        let Some(keyboard) = self.seat.get_keyboard() else {
            return;
        };
        let pressed = state == KeyState::Pressed;
        let shortcuts = self.shortcuts.clone();
        // Esc closes the open view: its own command is run.
        let switcher = self.switcher_active;
        let logo_released = std::cell::Cell::new(false);
        let vt = std::cell::Cell::new(None);
        let close_view = self.panorama_mode().map(|mode| match mode {
            crate::panorama::Mode::Panorama => silica_control::commands::PANORAMA_TOGGLE,
            crate::panorama::Mode::App => silica_control::commands::PANORAMA_APP,
            crate::panorama::Mode::Desktop => silica_control::commands::DESKTOP_SHOW,
        });
        let command = keyboard.input::<&'static str, _>(
            self,
            code,
            state,
            SERIAL_COUNTER.next_serial(),
            time,
            |_, modifiers, keysym| {
                let key = xkb::keysym_get_name(
                    keysym
                        .raw_latin_sym_or_raw_current_sym()
                        .unwrap_or(keysym.modified_sym()),
                );
                // Ctrl+Alt+Fn: another VT (only the real session has VTs).
                if let Some(n) = vt_switch(keysym.modified_sym()) {
                    if pressed {
                        vt.set(Some(n));
                    }
                    return FilterResult::Intercept(VT_SWITCH);
                }
                // App switcher: Esc cancels, the released Logo key confirms
                // (the release reaches the app anyway, which saw the press).
                if switcher {
                    if pressed && key == "Escape" {
                        return FilterResult::Intercept(silica_control::commands::SWITCHER_CANCEL);
                    }
                    if !pressed && (key == "Super_L" || key == "Super_R") {
                        logo_released.set(true);
                    }
                }
                if pressed
                    && key == "Escape"
                    && let Some(id) = close_view
                {
                    return FilterResult::Intercept(id);
                }
                match shortcuts.iter().find(|(shortcut, _)| {
                    pressed
                        && shortcut.matches(
                            modifiers.logo,
                            modifiers.ctrl,
                            modifiers.alt,
                            modifiers.shift,
                            &key,
                        )
                }) {
                    Some((_, id)) => FilterResult::Intercept(*id),
                    None => FilterResult::Forward,
                }
            },
        );
        if command == Some(VT_SWITCH) {
            if let Some(n) = vt.get() {
                self.change_vt(n);
            }
            return;
        }
        let command = match command {
            None if logo_released.get() => Some(silica_control::commands::SWITCHER_COMMIT),
            other => other,
        };
        if let Some(id) = command {
            let response = self.execute_command(id, serde_json::Value::Null);
            if let Err(err) = response {
                warn!("shortcut {id}: {err}");
            }
        }
    }

    /// The pointer is on a front layer that is not the menu bar (the Dock).
    fn over_dock_layer(&self, location: Point<f64, Logical>) -> bool {
        self.layer_under(&[Layer::Overlay, Layer::Top], location)
            .is_some_and(|(layer, _)| layer.namespace() != "menubar")
    }

    pub fn process_input_event<I: InputBackend>(&mut self, event: InputEvent<I>) {
        match event {
            InputEvent::Keyboard { event } => {
                self.handle_key(event.key_code(), event.state(), Event::time_msec(&event));
            }
            InputEvent::PointerMotionAbsolute { event } => {
                let Some(geometry) = self.space.output_geometry(&self.output) else {
                    return;
                };
                let location = event.position_transformed(geometry.size) + geometry.loc.to_f64();
                self.pointer_move_to(location, event.time_msec());
            }
            // Relative motion (a mouse or touchpad through libinput): from where the pointer
            // is, kept inside the output.
            InputEvent::PointerMotion { event } => {
                let (Some(pointer), Some(geometry)) = (
                    self.seat.get_pointer(),
                    self.space.output_geometry(&self.output),
                ) else {
                    return;
                };
                let location = clamp_to(pointer.current_location() + event.delta(), geometry);
                self.pointer_move_to(location, event.time_msec());
            }
            InputEvent::PointerButton { event } => {
                self.pointer_button(event.button_code(), event.state(), event.time_msec());
            }
            InputEvent::PointerAxis { event } => {
                let Some(pointer) = self.seat.get_pointer() else {
                    return;
                };
                // Scrolling outside an open transient that consumes it (#149).
                if self.scroll_on_transients(pointer.current_location()) {
                    return;
                }
                let source = event.source();
                let mut frame = AxisFrame::new(event.time_msec()).source(source);
                for axis in [Axis::Horizontal, Axis::Vertical] {
                    let v120 = event.amount_v120(axis);
                    let amount = event
                        .amount(axis)
                        .unwrap_or_else(|| v120.unwrap_or(0.0) * 15.0 / 120.0);
                    if amount != 0.0 {
                        frame = frame.value(axis, amount);
                        if let Some(discrete) = v120 {
                            frame = frame.v120(axis, discrete as i32);
                        }
                    } else if source == AxisSource::Finger {
                        frame = frame.stop(axis);
                    }
                }
                pointer.axis(self, frame);
                pointer.frame(self);
            }
            InputEvent::GestureSwipeBegin { event } => {
                self.gesture_begin(crate::gestures::GestureKind::Swipe, event.fingers());
            }
            InputEvent::GestureSwipeUpdate { event } => {
                let delta = event.delta();
                self.gesture_update(delta.x, delta.y, None);
            }
            InputEvent::GestureSwipeEnd { event } => {
                self.gesture_end(event.cancelled());
            }
            InputEvent::GesturePinchBegin { event } => {
                self.gesture_begin(crate::gestures::GestureKind::Pinch, event.fingers());
            }
            InputEvent::GesturePinchUpdate { event } => {
                self.gesture_update(0.0, 0.0, Some(event.scale()));
            }
            InputEvent::GesturePinchEnd { event } => {
                self.gesture_end(event.cancelled());
            }
            _ => {}
        }
    }
}

/// The marker returned by the key filter for a VT switch: not a command.
const VT_SWITCH: &str = "session.vt";

/// A point kept inside `area` (the last pixel included).
fn clamp_to(point: Point<f64, Logical>, area: Rectangle<i32, Logical>) -> Point<f64, Logical> {
    let (x0, y0) = (f64::from(area.loc.x), f64::from(area.loc.y));
    let x1 = x0 + f64::from(area.size.w.max(1)) - 1.0;
    let y1 = y0 + f64::from(area.size.h.max(1)) - 1.0;
    (point.x.clamp(x0, x1), point.y.clamp(y0, y1)).into()
}

/// The VT of the XF86Switch_VT_1…12 keysyms (Ctrl+Alt+F1…F12 in the default keymaps).
fn vt_switch(keysym: xkb::Keysym) -> Option<i32> {
    let first = xkb::Keysym::XF86_Switch_VT_1.raw();
    let last = xkb::Keysym::XF86_Switch_VT_12.raw();
    let raw = keysym.raw();
    (first..=last)
        .contains(&raw)
        .then(|| i32::try_from(raw - first + 1).ok())
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::{clamp_to, vt_switch};
    use smithay::input::keyboard::xkb;
    use smithay::utils::Rectangle;

    #[test]
    fn relative_motion_stays_on_the_output() {
        let area = Rectangle::new((0, 0).into(), (1280, 800).into());
        assert_eq!(clamp_to((-5.0, 900.0).into(), area), (0.0, 799.0).into());
        assert_eq!(clamp_to((10.5, 20.0).into(), area), (10.5, 20.0).into());
    }

    #[test]
    fn switch_keysyms_name_their_vt() {
        assert_eq!(vt_switch(xkb::Keysym::XF86_Switch_VT_1), Some(1));
        assert_eq!(vt_switch(xkb::Keysym::XF86_Switch_VT_12), Some(12));
        assert_eq!(vt_switch(xkb::Keysym::F1), None);
    }
}
