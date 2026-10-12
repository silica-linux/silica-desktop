//! Input: tastiera e puntatore verso le finestre.

use smithay::backend::input::{
    AbsolutePositionEvent, Axis, AxisSource, ButtonState, Event, InputBackend, InputEvent,
    KeyState, KeyboardKeyEvent, PointerAxisEvent, PointerButtonEvent,
};
use smithay::input::keyboard::{FilterResult, xkb};
use smithay::input::pointer::{AxisFrame, ButtonEvent, MotionEvent};
use smithay::utils::{Logical, Point, SERIAL_COUNTER};
use smithay::wayland::shell::wlr_layer::Layer;
use tracing::warn;

use crate::state::State;

impl State {
    /// Muove il puntatore in un punto del desktop (pixel logici).
    pub fn pointer_move_to(&mut self, location: Point<f64, Logical>, time: u32) {
        let Some(pointer) = self.seat.get_pointer() else {
            return;
        };
        let under = self.surface_under(location);
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

    /// Preme o rilascia un pulsante; la pressione dà il focus alla finestra sotto.
    pub fn pointer_button(&mut self, button: u32, state: ButtonState, time: u32) {
        let Some(pointer) = self.seat.get_pointer() else {
            return;
        };
        let serial = SERIAL_COUNTER.next_serial();
        if state == ButtonState::Pressed && !pointer.is_grabbed() {
            let location = pointer.current_location();
            match self.layer_under(&[Layer::Overlay, Layer::Top], location) {
                // Un clic su un layer non tocca le finestre; il focus solo se lo chiede.
                Some((layer, _)) => {
                    if layer.can_receive_keyboard_focus() {
                        self.focus_layer(&layer);
                    }
                }
                None => {
                    let clicked = self
                        .space
                        .element_under(location)
                        .map(|(window, _)| window.clone());
                    self.focus_window(clicked);
                }
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

    pub fn process_input_event<I: InputBackend>(&mut self, event: InputEvent<I>) {
        match event {
            InputEvent::Keyboard { event } => {
                let serial = SERIAL_COUNTER.next_serial();
                let time = Event::time_msec(&event);
                let Some(keyboard) = self.seat.get_keyboard() else {
                    return;
                };
                let pressed = event.state() == KeyState::Pressed;
                let shortcuts = self.shortcuts.clone();
                let command = keyboard.input::<&'static str, _>(
                    self,
                    event.key_code(),
                    event.state(),
                    serial,
                    time,
                    |_, modifiers, keysym| {
                        let key = xkb::keysym_get_name(
                            keysym
                                .raw_latin_sym_or_raw_current_sym()
                                .unwrap_or(keysym.modified_sym()),
                        );
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
                if let Some(id) = command {
                    let response = self.execute_command(id, serde_json::Value::Null);
                    if let Err(err) = response {
                        warn!("scorciatoia {id}: {err}");
                    }
                }
            }
            InputEvent::PointerMotionAbsolute { event } => {
                let Some(geometry) = self.space.output_geometry(&self.output) else {
                    return;
                };
                let location = event.position_transformed(geometry.size) + geometry.loc.to_f64();
                self.pointer_move_to(location, event.time_msec());
            }
            InputEvent::PointerButton { event } => {
                self.pointer_button(event.button_code(), event.state(), event.time_msec());
            }
            InputEvent::PointerAxis { event } => {
                let Some(pointer) = self.seat.get_pointer() else {
                    return;
                };
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
            _ => {}
        }
    }
}
