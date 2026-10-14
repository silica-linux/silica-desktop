//! From Wayland keys (evdev keycode + xkb keysym) to Flutter key events.
//!
//! Keys are computed as in the official GTK embedder: the physical one from the keycode
//! table, the logical one from the keysym table, or from the lowercase character for Latin-1
//! keysyms, or from the GTK plane for everything else.

use std::collections::HashMap;

use crate::ffi::{KeyEvent, KeyPhase};
use crate::keymap::{GTK_NUMPAD, GTK_TO_LOGICAL, LINUX_TO_PHYSICAL};
use crate::text_input::EditKey;

/// Plane of Flutter ids for keys without a table (GTK's key_mapping).
const GTK_PLANE: u64 = 0x015_0000_0000;

fn lookup(table: &[(u32, u64)], key: u32) -> Option<u64> {
    table
        .binary_search_by_key(&key, |(k, _)| *k)
        .ok()
        .and_then(|index| table.get(index))
        .map(|(_, value)| *value)
}

/// Lowercase of a Latin-1 keysym (the only ones whose keysym is the Unicode code point).
fn latin1_lower(keysym: u32) -> u32 {
    match keysym {
        0x41..=0x5a => keysym + 0x20,
        0xc0..=0xde if keysym != 0xd7 => keysym + 0x20,
        _ => keysym,
    }
}

pub fn physical_key(evdev: u32) -> u64 {
    let keycode = evdev.saturating_add(8);
    lookup(LINUX_TO_PHYSICAL, keycode).unwrap_or(GTK_PLANE | u64::from(keycode))
}

/// Super_L and Super_R: in Silica the Logo key is the apps' ⌘.
const SUPER_L: u32 = 0xffeb;
const SUPER_R: u32 = 0xffec;
/// Flutter's `LogicalKeyboardKey.metaLeft` and `metaRight`.
const META_LEFT: u64 = 0x0002_0000_0106;
const META_RIGHT: u64 = 0x0002_0000_0107;

pub fn logical_key(keysym: u32) -> u64 {
    // GTK calls them "Super" (logical `superKey`), which Flutter does not count as meta: the
    // apps' ⌘ shortcuts (SingleActivator(meta: true)) would never fire.
    match keysym {
        SUPER_L => return META_LEFT,
        SUPER_R => return META_RIGHT,
        _ => {}
    }
    if let Some(key) = lookup(GTK_TO_LOGICAL, keysym).or_else(|| lookup(GTK_NUMPAD, keysym)) {
        return key;
    }
    if keysym < 0x100 {
        return u64::from(latin1_lower(keysym));
    }
    GTK_PLANE | u64::from(keysym)
}

/// The text to pass to Flutter: no control characters (Enter, Esc, Backspace).
fn printable(utf8: Option<&str>) -> Option<String> {
    utf8.filter(|text| !text.is_empty() && !text.chars().any(char::is_control))
        .map(str::to_owned)
}

/// xkb keysyms used for editing (xkbcommon-keysyms.h).
mod keysym {
    pub const BACKSPACE: u32 = 0xff08;
    pub const RETURN: u32 = 0xff0d;
    pub const HOME: u32 = 0xff50;
    pub const LEFT: u32 = 0xff51;
    pub const RIGHT: u32 = 0xff53;
    pub const END: u32 = 0xff57;
    pub const KP_ENTER: u32 = 0xff8d;
    pub const DELETE: u32 = 0xffff;
}

/// What a key does in a text field, if the framework does not consume it.
pub fn edit_key(
    keysym: u32,
    utf8: Option<&str>,
    shift: bool,
    ctrl_or_alt: bool,
) -> Option<EditKey> {
    let extend = shift;
    match keysym {
        keysym::BACKSPACE => Some(EditKey::Backspace),
        keysym::DELETE => Some(EditKey::Delete),
        keysym::LEFT => Some(EditKey::Left { extend }),
        keysym::RIGHT => Some(EditKey::Right { extend }),
        keysym::HOME => Some(EditKey::Home { extend }),
        keysym::END => Some(EditKey::End { extend }),
        keysym::RETURN | keysym::KP_ENTER => Some(EditKey::Enter),
        _ if ctrl_or_alt => None,
        _ => printable(utf8).map(EditKey::Text),
    }
}

/// GTK modifier masks (GtkKeyHelper in raw_keyboard_linux.dart).
pub mod gtk_modifiers {
    pub const SHIFT: i64 = 1 << 0;
    pub const CAPS_LOCK: i64 = 1 << 1;
    pub const CONTROL: i64 = 1 << 2;
    pub const ALT: i64 = 1 << 3;
    pub const NUM_LOCK: i64 = 1 << 4;
    pub const META: i64 = 1 << 26;
}

/// The `flutter/keyevent` message (JSON) that goes with every key event: the
/// framework queues the `KeyEvent`s until this arrives, as with the GTK embedder.
pub fn raw_key_message(
    down: bool,
    evdev: u32,
    keysym: u32,
    utf8: Option<&str>,
    modifiers: i64,
) -> Vec<u8> {
    let unicode = printable(utf8)
        .and_then(|text| text.chars().next())
        .map_or(0, u32::from);
    serde_json::to_vec(&serde_json::json!({
        "type": if down { "keydown" } else { "keyup" },
        "keymap": "linux",
        "toolkit": "gtk",
        "scanCode": evdev.saturating_add(8),
        "keyCode": keysym,
        "modifiers": modifiers,
        "unicodeScalarValues": unicode,
    }))
    .unwrap_or_default()
}

/// The state of the pressed keys, as Flutter sees it: physical → logical.
#[derive(Debug, Default)]
pub struct Keyboard {
    pressed: HashMap<u64, u64>,
}

impl Keyboard {
    pub fn pressed(&self) -> &HashMap<u64, u64> {
        &self.pressed
    }

    pub fn press(&mut self, evdev: u32, keysym: u32, utf8: Option<&str>, micros: u64) -> KeyEvent {
        let physical = physical_key(evdev);
        let logical = logical_key(keysym);
        // A second press without a release is a repeat for Flutter.
        let phase = if self.pressed.insert(physical, logical).is_some() {
            KeyPhase::Repeat
        } else {
            KeyPhase::Down
        };
        KeyEvent {
            phase,
            physical,
            logical,
            character: printable(utf8),
            synthesized: false,
            timestamp_micros: micros,
        }
    }

    /// `None` if the key is not pressed: Flutter refuses a release without a press.
    pub fn release(&mut self, evdev: u32, micros: u64) -> Option<KeyEvent> {
        let physical = physical_key(evdev);
        let logical = self.pressed.remove(&physical)?;
        Some(KeyEvent {
            phase: KeyPhase::Up,
            physical,
            logical,
            character: None,
            synthesized: false,
            timestamp_micros: micros,
        })
    }

    /// Synthetic releases for the keys still pressed, when the window loses the focus.
    pub fn release_all(&mut self, micros: u64) -> Vec<KeyEvent> {
        self.pressed
            .drain()
            .map(|(physical, logical)| KeyEvent {
                phase: KeyPhase::Up,
                physical,
                logical,
                character: None,
                synthesized: true,
                timestamp_micros: micros,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_logo_key_is_meta_for_flutter() {
        // Without it, ⌘D, ⌘⌫ and the other app shortcuts never arrived (Mustache).
        assert_eq!(logical_key(SUPER_L), META_LEFT);
        assert_eq!(logical_key(SUPER_R), META_RIGHT);
        // The rest stays as in GTK.
        assert_eq!(logical_key(0xffe7), META_LEFT);
        assert_eq!(logical_key(u32::from(b'D')), u64::from(b'd'));
    }

    const KEY_A: u32 = 30;
    const KEY_ESC: u32 = 1;

    #[test]
    fn keys_match_flutter_tables() {
        // PhysicalKeyboardKey.keyA / LogicalKeyboardKey.keyA.
        assert_eq!(physical_key(KEY_A), 0x0007_0004);
        assert_eq!(logical_key(u32::from(b'a')), 0x61);
        // Uppercase: same logical key.
        assert_eq!(logical_key(u32::from(b'A')), 0x61);
        assert_eq!(physical_key(KEY_ESC), 0x0007_0029);
        assert_eq!(logical_key(0xff1b), 0x001_0000_001b);
        // Keysym without a table: GTK plane.
        assert_eq!(logical_key(0x0100_20ac), GTK_PLANE | 0x0100_20ac);
    }

    #[test]
    fn press_repeat_release_and_focus_loss() {
        let mut keyboard = Keyboard::default();
        let down = keyboard.press(KEY_A, u32::from(b'a'), Some("a"), 1);
        assert_eq!(
            (down.phase, down.character.as_deref()),
            (KeyPhase::Down, Some("a"))
        );
        assert_eq!(
            keyboard.press(KEY_A, u32::from(b'a'), Some("a"), 2).phase,
            KeyPhase::Repeat
        );
        assert_eq!(
            keyboard.release(KEY_A, 3).map(|e| e.phase),
            Some(KeyPhase::Up)
        );
        assert!(keyboard.release(KEY_A, 4).is_none());

        keyboard.press(KEY_ESC, 0xff1b, Some("\u{1b}"), 5);
        let released = keyboard.release_all(6);
        assert_eq!(released.len(), 1);
        assert!(released[0].synthesized && released[0].character.is_none());
    }

    #[test]
    fn raw_message_matches_gtk_format() {
        let message: serde_json::Value = serde_json::from_slice(&raw_key_message(
            true,
            KEY_A,
            0x41,
            Some("A"),
            gtk_modifiers::SHIFT,
        ))
        .unwrap();
        assert_eq!(message["type"], "keydown");
        assert_eq!(message["scanCode"], 38);
        assert_eq!(message["keyCode"], 0x41);
        assert_eq!(message["unicodeScalarValues"], 0x41);
        assert_eq!(message["modifiers"], 1);
        let up: serde_json::Value =
            serde_json::from_slice(&raw_key_message(false, KEY_ESC, 0xff1b, None, 0)).unwrap();
        assert_eq!(
            (up["type"].as_str(), up["unicodeScalarValues"].as_i64()),
            (Some("keyup"), Some(0))
        );
    }

    #[test]
    fn edit_keys() {
        assert_eq!(
            edit_key(0xff08, Some("\u{8}"), false, false),
            Some(EditKey::Backspace)
        );
        assert_eq!(
            edit_key(0xff51, None, true, false),
            Some(EditKey::Left { extend: true })
        );
        assert_eq!(
            edit_key(0x61, Some("a"), false, false),
            Some(EditKey::Text("a".into()))
        );
        assert_eq!(edit_key(0x61, Some("a"), false, true), None);
        assert_eq!(edit_key(0xff1b, Some("\u{1b}"), false, false), None);
    }
}
