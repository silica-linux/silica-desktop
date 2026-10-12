//! Catalogo dei comandi di Silica: la stessa lista per UI, CLI, canale di controllo e MCP.
//!
//! Ogni comando ha id, etichetta, descrizione, scorciatoia facoltativa e lo schema JSON
//! dei parametri. Chi esegue i comandi (Ossido) deve gestire ogni id del catalogo: un
//! test in `crates/ossido/tests/` lo verifica.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Cattura l'output come PNG.
pub const UI_SCREENSHOT: &str = "ui.screenshot";
/// Elenca il catalogo dei comandi.
pub const ENGINE_COMMANDS: &str = "engine.commands";
/// Esegue un comando del catalogo per id: `{"id": "...", "params": {...}}`.
pub const ENGINE_EXECUTE: &str = "engine.execute";
pub const WINDOW_LIST: &str = "window.list";
pub const WINDOW_FOCUS: &str = "window.focus";
pub const WINDOW_CLOSE: &str = "window.close";
pub const WINDOW_MOVE: &str = "window.move";
pub const APP_LAUNCH: &str = "app.launch";
pub const SESSION_QUIT: &str = "session.quit";
pub const INPUT_POINTER_MOVE: &str = "input.pointer_move";
pub const INPUT_POINTER_BUTTON: &str = "input.pointer_button";
pub const INPUT_CLICK: &str = "input.click";
pub const INPUT_KEY: &str = "input.key";
pub const INPUT_TYPE: &str = "input.type";
pub const LAYER_LIST: &str = "layer.list";

#[derive(Debug, Clone, Copy)]
pub struct CommandSpec {
    pub id: &'static str,
    /// Etichetta per l'interfaccia (menu, Faro).
    pub label: &'static str,
    pub description: &'static str,
    /// Scorciatoia nel formato di [`Shortcut::parse`], per esempio `Logo+Shift+Q`.
    pub shortcut: Option<&'static str>,
    /// Schema JSON dei parametri.
    pub params: fn() -> Value,
}

impl CommandSpec {
    pub fn to_json(&self) -> Value {
        json!({
            "id": self.id,
            "label": self.label,
            "description": self.description,
            "shortcut": self.shortcut,
            "params": (self.params)(),
        })
    }
}

fn no_params() -> Value {
    json!({"type": "object", "properties": {}, "additionalProperties": false})
}

fn window_target() -> Value {
    json!({
        "type": "object",
        "properties": {
            "id": {"type": "integer", "minimum": 0, "description": "Finestra; senza, quella con il focus"}
        },
        "additionalProperties": false
    })
}

fn point() -> Value {
    json!({
        "type": "object",
        "properties": {
            "x": {"type": "number", "description": "Pixel logici dal bordo sinistro dell'output"},
            "y": {"type": "number", "description": "Pixel logici dal bordo superiore dell'output"},
            "button": {"type": "string", "enum": ["left", "right", "middle"]}
        },
        "required": ["x", "y"],
        "additionalProperties": false
    })
}

pub const CATALOG: &[CommandSpec] = &[
    CommandSpec {
        id: UI_SCREENSHOT,
        label: "Istantanea dello schermo",
        description: "Cattura l'output come PNG, in un file o in base64.",
        shortcut: None,
        params: || {
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Dove scrivere il PNG; senza, torna in png_base64"}
                },
                "additionalProperties": false
            })
        },
    },
    CommandSpec {
        id: ENGINE_COMMANDS,
        label: "Elenca i comandi",
        description: "Restituisce il catalogo dei comandi con parametri e scorciatoie.",
        shortcut: None,
        params: no_params,
    },
    CommandSpec {
        id: ENGINE_EXECUTE,
        label: "Esegui comando",
        description: "Esegue un comando del catalogo per id.",
        shortcut: None,
        params: || {
            json!({
                "type": "object",
                "properties": {
                    "id": {"type": "string"},
                    "params": {"type": "object"}
                },
                "required": ["id"],
                "additionalProperties": false
            })
        },
    },
    CommandSpec {
        id: WINDOW_LIST,
        label: "Elenca le finestre",
        description: "Finestre aperte, dall'alto in basso, con titolo, app e geometria.",
        shortcut: None,
        params: no_params,
    },
    CommandSpec {
        id: LAYER_LIST,
        label: "Elenca i layer",
        description: "Superfici della shell (wlr-layer-shell): namespace, livello, geometria, zona esclusiva.",
        shortcut: None,
        params: no_params,
    },
    CommandSpec {
        id: WINDOW_FOCUS,
        label: "Porta in primo piano",
        description: "Dà il focus a una finestra e la porta sopra le altre.",
        shortcut: None,
        params: window_target,
    },
    CommandSpec {
        id: WINDOW_CLOSE,
        label: "Chiudi finestra",
        description: "Chiede alla finestra di chiudersi.",
        shortcut: Some("Logo+W"),
        params: window_target,
    },
    CommandSpec {
        id: WINDOW_MOVE,
        label: "Sposta finestra",
        description: "Sposta una finestra alla posizione data, in pixel logici.",
        shortcut: None,
        params: || {
            json!({
                "type": "object",
                "properties": {
                    "id": {"type": "integer", "minimum": 0},
                    "x": {"type": "integer"},
                    "y": {"type": "integer"}
                },
                "required": ["x", "y"],
                "additionalProperties": false
            })
        },
    },
    CommandSpec {
        id: APP_LAUNCH,
        label: "Apri app",
        description: "Avvia un comando come client di questa sessione.",
        shortcut: None,
        params: || {
            json!({
                "type": "object",
                "properties": {"command": {"type": "string"}},
                "required": ["command"],
                "additionalProperties": false
            })
        },
    },
    CommandSpec {
        id: SESSION_QUIT,
        label: "Esci dalla sessione",
        description: "Chiude Ossido e tutte le finestre.",
        shortcut: Some("Logo+Shift+Q"),
        params: no_params,
    },
    CommandSpec {
        id: INPUT_POINTER_MOVE,
        label: "Muovi il puntatore",
        description: "Porta il puntatore in un punto dell'output, come un mouse vero.",
        shortcut: None,
        params: || {
            json!({
                "type": "object",
                "properties": {"x": {"type": "number"}, "y": {"type": "number"}},
                "required": ["x", "y"],
                "additionalProperties": false
            })
        },
    },
    CommandSpec {
        id: INPUT_POINTER_BUTTON,
        label: "Premi o rilascia un pulsante",
        description: "Preme o rilascia un pulsante del mouse dove si trova il puntatore.",
        shortcut: None,
        params: || {
            json!({
                "type": "object",
                "properties": {
                    "button": {"type": "string", "enum": ["left", "right", "middle"]},
                    "pressed": {"type": "boolean"}
                },
                "required": ["pressed"],
                "additionalProperties": false
            })
        },
    },
    CommandSpec {
        id: INPUT_CLICK,
        label: "Clic",
        description: "Muove il puntatore in un punto e fa clic (pulsante sinistro se non indicato).",
        shortcut: None,
        params: point,
    },
    CommandSpec {
        id: INPUT_KEY,
        label: "Premi un tasto",
        description: "Preme e rilascia un tasto per nome xkb (Return, BackSpace, Escape, Left...) o carattere; con pressed solo pressione o rilascio.",
        shortcut: None,
        params: || {
            json!({
                "type": "object",
                "properties": {
                    "key": {"type": "string"},
                    "pressed": {"type": "boolean"}
                },
                "required": ["key"],
                "additionalProperties": false
            })
        },
    },
    CommandSpec {
        id: INPUT_TYPE,
        label: "Scrivi testo",
        description: "Scrive un testo tasto per tasto nella finestra con il focus.",
        shortcut: None,
        params: || {
            json!({
                "type": "object",
                "properties": {"text": {"type": "string"}},
                "required": ["text"],
                "additionalProperties": false
            })
        },
    },
];

pub fn find(id: &str) -> Option<&'static CommandSpec> {
    CATALOG.iter().find(|spec| spec.id == id)
}

pub fn catalog_json() -> Value {
    Value::Array(CATALOG.iter().map(CommandSpec::to_json).collect())
}

/// Parametri di [`ENGINE_EXECUTE`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecuteParams {
    pub id: String,
    #[serde(default)]
    pub params: Value,
}

/// Parametri dei comandi che agiscono su una finestra.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowTarget {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowMove {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppLaunch {
    pub command: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MouseButton {
    #[default]
    Left,
    Right,
    Middle,
}

impl MouseButton {
    /// Codice evdev del pulsante (linux/input-event-codes.h).
    pub fn code(self) -> u32 {
        match self {
            MouseButton::Left => 0x110,
            MouseButton::Right => 0x111,
            MouseButton::Middle => 0x112,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PointerMove {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PointerButton {
    #[serde(default)]
    pub button: MouseButton,
    pub pressed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Click {
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub button: MouseButton,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Key {
    pub key: String,
    /// Assente: pressione e rilascio.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressed: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypeText {
    pub text: String,
}

/// Un layer in [`LAYER_LIST`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayerInfo {
    pub namespace: String,
    /// `background`, `bottom`, `top` o `overlay`.
    pub layer: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub exclusive_zone: i32,
    pub focused: bool,
}

/// Una finestra in [`WINDOW_LIST`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowInfo {
    pub id: u64,
    pub title: Option<String>,
    pub app_id: Option<String>,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub focused: bool,
}

/// Una scorciatoia da tastiera: modificatori più il nome del keysym xkb.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Shortcut {
    pub logo: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    /// Nome del keysym senza modificatori, confrontato senza badare alle maiuscole.
    pub key: String,
}

impl Shortcut {
    /// Legge `Logo+Shift+Q`: modificatori (`Logo`, `Ctrl`, `Alt`, `Shift`) e un tasto finale.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut shortcut = Shortcut::default();
        let mut parts = text.split('+').map(str::trim).peekable();
        while let Some(part) = parts.next() {
            if parts.peek().is_none() {
                if part.is_empty() {
                    return Err(format!("scorciatoia senza tasto: {text}"));
                }
                shortcut.key = part.to_owned();
                return Ok(shortcut);
            }
            let flag = match part.to_ascii_lowercase().as_str() {
                "logo" | "super" => &mut shortcut.logo,
                "ctrl" | "control" => &mut shortcut.ctrl,
                "alt" => &mut shortcut.alt,
                "shift" => &mut shortcut.shift,
                other => return Err(format!("modificatore sconosciuto: {other}")),
            };
            *flag = true;
        }
        Err(format!("scorciatoia vuota: {text:?}"))
    }

    pub fn matches(&self, logo: bool, ctrl: bool, alt: bool, shift: bool, key: &str) -> bool {
        self.logo == logo
            && self.ctrl == ctrl
            && self.alt == alt
            && self.shift == shift
            && self.key.eq_ignore_ascii_case(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_ids_are_unique_and_documented() {
        let mut ids: Vec<_> = CATALOG.iter().map(|spec| spec.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), CATALOG.len());
        for spec in CATALOG {
            assert!(
                !spec.label.is_empty() && !spec.description.is_empty(),
                "{}",
                spec.id
            );
            assert_eq!((spec.params)()["type"], "object", "{}", spec.id);
        }
    }

    #[test]
    fn every_shortcut_parses_and_none_collide() {
        let shortcuts: Vec<_> = CATALOG
            .iter()
            .filter_map(|spec| spec.shortcut)
            .map(|text| Shortcut::parse(text).unwrap())
            .collect();
        for (i, a) in shortcuts.iter().enumerate() {
            for b in shortcuts.iter().skip(i + 1) {
                assert_ne!(a, b);
            }
        }
    }

    #[test]
    fn shortcut_parse_and_match() {
        let shortcut = Shortcut::parse("Logo+Shift+Q").unwrap();
        assert!(shortcut.matches(true, false, false, true, "q"));
        assert!(!shortcut.matches(true, false, false, false, "q"));
        assert!(Shortcut::parse("Logo+").is_err());
        assert!(Shortcut::parse("Hyper+Q").is_err());
    }

    #[test]
    fn window_target_rejects_unknown_fields() {
        assert!(serde_json::from_value::<WindowTarget>(json!({"idd": 1})).is_err());
        assert_eq!(
            serde_json::from_value::<WindowTarget>(json!({})).unwrap(),
            WindowTarget::default()
        );
    }
}
