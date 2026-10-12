//! Input: tastiera e puntatore verso le finestre.

use smithay::backend::input::{
    AbsolutePositionEvent, Axis, AxisSource, ButtonState, Event, InputBackend, InputEvent,
    KeyState, KeyboardKeyEvent, PointerAxisEvent, PointerButtonEvent,
};
use smithay::input::keyboard::{FilterResult, xkb};
use smithay::input::pointer::{AxisFrame, ButtonEvent, MotionEvent};
use smithay::utils::SERIAL_COUNTER;
use tracing::warn;

use crate::state::State;

impl State {
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
                        time: event.time_msec(),
                    },
                );
                pointer.frame(self);
            }
            InputEvent::PointerButton { event } => {
                let Some(pointer) = self.seat.get_pointer() else {
                    return;
                };
                let serial = SERIAL_COUNTER.next_serial();
                let state = event.state();
                if state == ButtonState::Pressed && !pointer.is_grabbed() {
                    let clicked = self
                        .space
                        .element_under(pointer.current_location())
                        .map(|(window, _)| window.clone());
                    self.focus_window(clicked);
                }
                pointer.button(
                    self,
                    &ButtonEvent {
                        button: event.button_code(),
                        state,
                        serial,
                        time: event.time_msec(),
                    },
                );
                pointer.frame(self);
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
