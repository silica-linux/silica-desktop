//! Tasti sintetici per `input.key` e `input.type`: da nome o carattere a keycode.
//!
//! Si compila una keymap xkb con la stessa configurazione predefinita della tastiera di
//! Ossido e se ne ricava la mappa inversa keysym → (keycode, servono Maiusc).

use std::collections::HashMap;

use smithay::input::keyboard::xkb;

/// Keycode xkb (evdev + 8) del tasto Maiusc sinistro.
pub const SHIFT_LEFT: u32 = 50;

pub struct KeyLookup {
    by_keysym: HashMap<u32, (u32, bool)>,
}

impl KeyLookup {
    pub fn new() -> Result<Self, String> {
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let keymap = xkb::Keymap::new_from_names(&context, "", "", "", "", None, 0)
            .ok_or("keymap xkb non compilabile")?;
        let mut by_keysym = HashMap::new();
        let (min, max) = (keymap.min_keycode().raw(), keymap.max_keycode().raw());
        // Prima il livello base, poi quello con Maiusc: vince il tasto più semplice.
        for (level, shift) in [(0, false), (1, true)] {
            for code in min..=max {
                for sym in keymap.key_get_syms_by_level(xkb::Keycode::new(code), 0, level) {
                    by_keysym.entry(sym.raw()).or_insert((code, shift));
                }
            }
        }
        Ok(Self { by_keysym })
    }

    /// Keycode e Maiusc per un nome di keysym (`Return`, `BackSpace`, `a`) o un carattere.
    pub fn find(&self, key: &str) -> Option<(u32, bool)> {
        let mut chars = key.chars();
        let keysym = match (chars.next(), chars.next()) {
            (Some(ch), None) => xkb::utf32_to_keysym(u32::from(ch)),
            _ => xkb::keysym_from_name(key, xkb::KEYSYM_NO_FLAGS),
        };
        self.by_keysym.get(&keysym.raw()).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_keys_by_name_and_character() {
        let keys = KeyLookup::new().unwrap();
        // Keycode xkb = evdev + 8: KEY_A = 30, KEY_ENTER = 28.
        assert_eq!(keys.find("a"), Some((38, false)));
        assert_eq!(keys.find("A"), Some((38, true)));
        assert_eq!(keys.find("Return"), Some((36, false)));
        assert_eq!(keys.find(" "), Some((65, false)));
        assert_eq!(keys.find("Shift_L"), Some((SHIFT_LEFT, false)));
        assert_eq!(keys.find("NonEsiste"), None);
    }
}

/// Un tasto che diventa il tasto Logo (Super sinistro) prima di tutto il resto: con Ossido
/// annidato in GNOME o in KDE le combinazioni con Super le prende il desktop di sotto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogoKey {
    None,
    RightCtrl,
    RightAlt,
}

/// Codici xkb (evdev + 8).
const KEY_RIGHTCTRL: u32 = 97 + 8;
const KEY_RIGHTALT: u32 = 100 + 8;
const KEY_LEFTMETA: u32 = 125 + 8;

impl LogoKey {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "none" => Some(Self::None),
            "rightctrl" => Some(Self::RightCtrl),
            "rightalt" => Some(Self::RightAlt),
            _ => None,
        }
    }

    pub fn remap(
        self,
        code: smithay::input::keyboard::Keycode,
    ) -> smithay::input::keyboard::Keycode {
        let from = match self {
            Self::None => return code,
            Self::RightCtrl => KEY_RIGHTCTRL,
            Self::RightAlt => KEY_RIGHTALT,
        };
        if code.raw() == from {
            KEY_LEFTMETA.into()
        } else {
            code
        }
    }
}

#[cfg(test)]
mod logo_tests {
    use super::*;

    #[test]
    fn the_logo_key_is_remapped_only_when_chosen() {
        let ctrl_r = smithay::input::keyboard::Keycode::new(KEY_RIGHTCTRL);
        assert_eq!(LogoKey::RightCtrl.remap(ctrl_r).raw(), KEY_LEFTMETA);
        assert_eq!(LogoKey::None.remap(ctrl_r).raw(), KEY_RIGHTCTRL);
        let a = smithay::input::keyboard::Keycode::new(38);
        assert_eq!(LogoKey::RightCtrl.remap(a).raw(), 38);
        assert_eq!(LogoKey::parse("rightalt"), Some(LogoKey::RightAlt));
        assert_eq!(LogoKey::parse("x"), None);
    }
}
