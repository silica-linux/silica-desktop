//! Catalogo dei comandi di Silica: la stessa lista per UI, CLI, canale di controllo e MCP.
//!
//! Ogni comando ha id, etichetta, descrizione, scorciatoia facoltativa e lo schema JSON
//! dei parametri. Chi esegue i comandi (Ossido) deve gestire ogni id del catalogo: un
//! test in `crates/ossido/tests/` lo verifica.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Cattura l'output come PNG.
pub const UI_SCREENSHOT: &str = "ui.screenshot";
pub const UI_ANIMATION_SCALE: &str = "ui.animation_scale";
pub const UI_GLASS_QUALITY: &str = "ui.glass_quality";
pub const PERF_RENDER: &str = "perf.render";
/// Elenca il catalogo dei comandi.
pub const ENGINE_COMMANDS: &str = "engine.commands";
/// Esegue un comando del catalogo per id: `{"id": "...", "params": {...}}`.
pub const ENGINE_EXECUTE: &str = "engine.execute";
pub const WINDOW_LIST: &str = "window.list";
pub const WINDOW_FOCUS: &str = "window.focus";
pub const WINDOW_CLOSE: &str = "window.close";
pub const WINDOW_MOVE: &str = "window.move";
pub const WINDOW_MINIMIZE: &str = "window.minimize";
pub const WINDOW_FULLSCREEN: &str = "window.fullscreen";
pub const WINDOW_TILE_LEFT: &str = "window.tile_left";
pub const WINDOW_TILE_RIGHT: &str = "window.tile_right";
pub const WINDOW_TILE_TOP: &str = "window.tile_top";
pub const WINDOW_TILE_BOTTOM: &str = "window.tile_bottom";
pub const WINDOW_TILE_FILL: &str = "window.tile_fill";
pub const WINDOW_ARRANGE_2: &str = "window.arrange_2";
pub const WINDOW_ARRANGE_3: &str = "window.arrange_3";
pub const WINDOW_ARRANGE_4: &str = "window.arrange_4";
pub const APP_LAUNCH: &str = "app.launch";
pub const SESSION_QUIT: &str = "session.quit";
pub const APP_QUIT: &str = "app.quit";
pub const INPUT_POINTER_MOVE: &str = "input.pointer_move";
pub const INPUT_POINTER_BUTTON: &str = "input.pointer_button";
pub const INPUT_CLICK: &str = "input.click";
pub const INPUT_KEY: &str = "input.key";
pub const INPUT_TYPE: &str = "input.type";
pub const INPUT_GESTURE: &str = "input.gesture";
pub const LAYER_LIST: &str = "layer.list";
pub const PERF_FRAMES: &str = "perf.frames";
pub const PERF_PROCESS: &str = "perf.process";
pub const GLASS_LIST: &str = "glass.list";
pub const FARO_TOGGLE: &str = "faro.toggle";
pub const NOTIFICATIONS_TOGGLE: &str = "notifications.toggle";
pub const LAUNCHER_TOGGLE: &str = "launcher.toggle";
pub const SWITCHER_NEXT: &str = "switcher.next";
pub const SWITCHER_PREVIOUS: &str = "switcher.previous";
pub const SWITCHER_COMMIT: &str = "switcher.commit";
pub const SWITCHER_CANCEL: &str = "switcher.cancel";
pub const CONTROLCENTER_TOGGLE: &str = "controlcenter.toggle";
pub const SYSTEM_STATUS: &str = "system.status";
pub const SPACE_LIST: &str = "space.list";
pub const SPACE_NEXT: &str = "space.next";
pub const SPACE_PREVIOUS: &str = "space.previous";
pub const SPACE_GOTO: &str = "space.goto";
pub const SPACE_ADD: &str = "space.add";
pub const SPACE_REMOVE: &str = "space.remove";
pub const WINDOW_MOVE_TO_SPACE: &str = "window.move_to_space";
pub const PANORAMA_TOGGLE: &str = "panorama.toggle";
pub const PANORAMA_STATE: &str = "panorama.state";
pub const PANORAMA_SPACES_BAR: &str = "panorama.spaces_bar";
pub const PANORAMA_APP: &str = "panorama.app";
pub const DESKTOP_SHOW: &str = "desktop.show";
pub const SYSTEM_WIFI: &str = "system.wifi";
pub const SYSTEM_BLUETOOTH: &str = "system.bluetooth";
pub const SYSTEM_VOLUME: &str = "system.volume";
pub const SYSTEM_VOLUME_UP: &str = "system.volume_up";
pub const SYSTEM_VOLUME_DOWN: &str = "system.volume_down";
pub const SYSTEM_VOLUME_UP_FINE: &str = "system.volume_up_fine";
pub const SYSTEM_VOLUME_DOWN_FINE: &str = "system.volume_down_fine";
pub const SYSTEM_BRIGHTNESS_UP_FINE: &str = "system.brightness_up_fine";
pub const SYSTEM_BRIGHTNESS_DOWN_FINE: &str = "system.brightness_down_fine";
pub const SYSTEM_MUTE: &str = "system.mute";
pub const SYSTEM_MUTE_TOGGLE: &str = "system.mute_toggle";
pub const SYSTEM_BRIGHTNESS: &str = "system.brightness";
pub const SYSTEM_BRIGHTNESS_UP: &str = "system.brightness_up";
pub const SYSTEM_BRIGHTNESS_DOWN: &str = "system.brightness_down";
pub const SYSTEM_DND: &str = "system.dnd";

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

fn index_param() -> Value {
    json!({
        "type": "object",
        "properties": {"index": {"type": "integer", "minimum": 0}},
        "required": ["index"],
        "additionalProperties": false
    })
}

fn enabled_param() -> Value {
    json!({
        "type": "object",
        "properties": {"enabled": {"type": "boolean"}},
        "required": ["enabled"],
        "additionalProperties": false
    })
}

fn value_param() -> Value {
    json!({
        "type": "object",
        "properties": {"value": {"type": "number", "minimum": 0, "maximum": 1}},
        "required": ["value"],
        "additionalProperties": false
    })
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
        id: UI_ANIMATION_SCALE,
        label: "Velocità delle animazioni",
        description: "Moltiplica le durate delle animazioni; 0 le spegne (riduci movimento).",
        shortcut: None,
        params: || {
            json!({
                "type": "object",
                "properties": {"scale": {"type": "number", "minimum": 0, "maximum": 1000}},
                "required": ["scale"],
                "additionalProperties": false
            })
        },
    },
    CommandSpec {
        id: UI_GLASS_QUALITY,
        label: "Qualità del vetro",
        description: "Vetro pieno, smerigliato statico (solo lo sfondo del desktop) o opaco; auto sceglie in base alla GPU.",
        shortcut: None,
        params: || {
            json!({
                "type": "object",
                "properties": {"quality": {"type": "string", "enum": ["auto", "full", "frosted", "opaque"]}},
                "required": ["quality"],
                "additionalProperties": false
            })
        },
    },
    CommandSpec {
        id: PERF_RENDER,
        label: "Tempi di rendering",
        description: "Durata del rendering di Ossido per frame (backend winit): media, percentili, massimo.",
        shortcut: None,
        params: || {
            json!({
                "type": "object",
                "properties": {"reset": {"type": "boolean"}},
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
        id: LAUNCHER_TOGGLE,
        label: "App",
        description: "Apre o chiude il launcher delle app: tutte le app in una griglia per categoria, con le recenti in cima (componente della shell).",
        shortcut: None,
        params: no_params,
    },
    CommandSpec {
        id: SWITCHER_NEXT,
        label: "App successiva",
        description: "Apre il selettore delle app sull'app usata prima, o passa alla successiva; al rilascio del tasto Logo si attiva l'app scelta (componente della shell).",
        shortcut: Some("Logo+Tab"),
        params: no_params,
    },
    CommandSpec {
        id: SWITCHER_PREVIOUS,
        label: "App precedente",
        description: "Come switcher.next, all'indietro.",
        shortcut: Some("Logo+Shift+Tab"),
        params: no_params,
    },
    CommandSpec {
        id: SWITCHER_COMMIT,
        label: "Attiva l'app scelta",
        description: "Chiude il selettore delle app attivando l'app scelta (il rilascio del tasto Logo).",
        shortcut: None,
        params: no_params,
    },
    CommandSpec {
        id: SWITCHER_CANCEL,
        label: "Annulla il selettore",
        description: "Chiude il selettore delle app senza cambiare app (Esc).",
        shortcut: None,
        params: no_params,
    },
    CommandSpec {
        id: FARO_TOGGLE,
        label: "Faro",
        description: "Apre o chiude Faro, la ricerca delle app (componente della shell).",
        shortcut: Some("Logo+space"),
        params: no_params,
    },
    CommandSpec {
        id: NOTIFICATIONS_TOGGLE,
        label: "Centro notifiche",
        description: "Apre o chiude il Centro notifiche (componente della shell; clic sull'orologio).",
        shortcut: None,
        params: no_params,
    },
    CommandSpec {
        id: CONTROLCENTER_TOGGLE,
        label: "Centro di controllo",
        description: "Apre o chiude il Centro di controllo (componente della shell; icona nella barra).",
        shortcut: None,
        params: no_params,
    },
    CommandSpec {
        id: SPACE_LIST,
        label: "Elenca gli spazi",
        description: "Gli spazi di lavoro: quale è attivo e gli id delle finestre di ciascuno.",
        shortcut: None,
        params: no_params,
    },
    CommandSpec {
        id: SPACE_NEXT,
        label: "Spazio successivo",
        description: "Passa allo spazio a destra (oltre l'ultimo non fa niente).",
        shortcut: Some("Ctrl+Right"),
        params: no_params,
    },
    CommandSpec {
        id: SPACE_PREVIOUS,
        label: "Spazio precedente",
        description: "Passa allo spazio a sinistra (prima del primo non fa niente).",
        shortcut: Some("Ctrl+Left"),
        params: no_params,
    },
    CommandSpec {
        id: SPACE_GOTO,
        label: "Vai allo spazio",
        description: "Passa allo spazio `index` (da 0).",
        shortcut: None,
        params: index_param,
    },
    CommandSpec {
        id: SPACE_ADD,
        label: "Nuovo spazio",
        description: "Aggiunge uno spazio vuoto in fondo; restituisce il suo indice.",
        shortcut: None,
        params: no_params,
    },
    CommandSpec {
        id: SPACE_REMOVE,
        label: "Togli lo spazio",
        description: "Toglie lo spazio `index`; le sue finestre passano a quello prima.",
        shortcut: None,
        params: index_param,
    },
    CommandSpec {
        id: WINDOW_MOVE_TO_SPACE,
        label: "Sposta nello spazio",
        description: "Sposta la finestra `id` (o quella con il focus) nello spazio `index`.",
        shortcut: None,
        params: || {
            json!({
                "type": "object",
                "properties": {
                    "id": {"type": "integer", "minimum": 0},
                    "index": {"type": "integer", "minimum": 0}
                },
                "required": ["index"],
                "additionalProperties": false
            })
        },
    },
    CommandSpec {
        id: PANORAMA_TOGGLE,
        label: "Panorama",
        description: "Mostra tutte le finestre dello spazio una accanto all'altra con la barra degli spazi in alto; se è aperto lo chiude.",
        shortcut: Some("Ctrl+Up"),
        params: no_params,
    },
    CommandSpec {
        id: PANORAMA_APP,
        label: "Finestre dell'app",
        description: "Mostra le finestre dell'app in primo piano al centro, con il titolo sotto; se è già aperto lo chiude.",
        shortcut: Some("Ctrl+Down"),
        params: no_params,
    },
    CommandSpec {
        id: DESKTOP_SHOW,
        label: "Mostra la scrivania",
        description: "Sposta le finestre fuori dallo schermo per mostrare la scrivania; di nuovo (o un clic) le rimette al loro posto.",
        shortcut: Some("F11"),
        params: no_params,
    },
    CommandSpec {
        id: PANORAMA_SPACES_BAR,
        label: "Barra degli spazi",
        description: "Con Panorama aperto, espande (`enabled`) o richiude la barra degli spazi con le miniature, come il puntatore in alto.",
        shortcut: None,
        params: enabled_param,
    },
    CommandSpec {
        id: PANORAMA_STATE,
        label: "Stato di Panorama",
        description: "Se Panorama è aperto, l'avanzamento dell'animazione (0–1) e le cornici disposte delle finestre.",
        shortcut: None,
        params: no_params,
    },
    CommandSpec {
        id: SYSTEM_STATUS,
        label: "Stato del sistema",
        description: "Wi-Fi, Bluetooth, batteria, luminosità, volume e Non disturbare, da silica-daemon (chiavi di dev.frascella.Silica.System1).",
        shortcut: None,
        params: no_params,
    },
    CommandSpec {
        id: SYSTEM_WIFI,
        label: "Wi-Fi",
        description: "Accende o spegne il Wi-Fi.",
        shortcut: None,
        params: enabled_param,
    },
    CommandSpec {
        id: SYSTEM_BLUETOOTH,
        label: "Bluetooth",
        description: "Accende o spegne il Bluetooth.",
        shortcut: None,
        params: enabled_param,
    },
    CommandSpec {
        id: SYSTEM_VOLUME,
        label: "Volume",
        description: "Volume dell'uscita predefinita, da 0 a 1.",
        shortcut: None,
        params: value_param,
    },
    CommandSpec {
        id: SYSTEM_VOLUME_UP,
        label: "Alza il volume",
        description: "Alza il volume di un sedicesimo (toglie il muto).",
        shortcut: Some("XF86AudioRaiseVolume"),
        params: no_params,
    },
    CommandSpec {
        id: SYSTEM_VOLUME_DOWN,
        label: "Abbassa il volume",
        description: "Abbassa il volume di un sedicesimo.",
        shortcut: Some("XF86AudioLowerVolume"),
        params: no_params,
    },
    CommandSpec {
        id: SYSTEM_VOLUME_UP_FINE,
        label: "Alza il volume di poco",
        description: "Alza il volume di un quarto di passo (1/64).",
        shortcut: Some("Alt+Shift+XF86AudioRaiseVolume"),
        params: no_params,
    },
    CommandSpec {
        id: SYSTEM_VOLUME_DOWN_FINE,
        label: "Abbassa il volume di poco",
        description: "Abbassa il volume di un quarto di passo (1/64).",
        shortcut: Some("Alt+Shift+XF86AudioLowerVolume"),
        params: no_params,
    },
    CommandSpec {
        id: SYSTEM_MUTE,
        label: "Muto",
        description: "Toglie o rimette l'audio dell'uscita predefinita.",
        shortcut: None,
        params: enabled_param,
    },
    CommandSpec {
        id: SYSTEM_MUTE_TOGGLE,
        label: "Muto sì/no",
        description: "Inverte il muto dell'uscita predefinita.",
        shortcut: Some("XF86AudioMute"),
        params: no_params,
    },
    CommandSpec {
        id: SYSTEM_BRIGHTNESS,
        label: "Luminosità",
        description: "Luminosità dello schermo, da 0 a 1 (mai spento del tutto).",
        shortcut: None,
        params: value_param,
    },
    CommandSpec {
        id: SYSTEM_BRIGHTNESS_UP,
        label: "Aumenta la luminosità",
        description: "Aumenta la luminosità di un sedicesimo.",
        shortcut: Some("XF86MonBrightnessUp"),
        params: no_params,
    },
    CommandSpec {
        id: SYSTEM_BRIGHTNESS_DOWN,
        label: "Diminuisci la luminosità",
        description: "Diminuisce la luminosità di un sedicesimo.",
        shortcut: Some("XF86MonBrightnessDown"),
        params: no_params,
    },
    CommandSpec {
        id: SYSTEM_BRIGHTNESS_UP_FINE,
        label: "Aumenta la luminosità di poco",
        description: "Aumenta la luminosità di un quarto di passo (1/64).",
        shortcut: Some("Alt+Shift+XF86MonBrightnessUp"),
        params: no_params,
    },
    CommandSpec {
        id: SYSTEM_BRIGHTNESS_DOWN_FINE,
        label: "Diminuisci la luminosità di poco",
        description: "Diminuisce la luminosità di un quarto di passo (1/64).",
        shortcut: Some("Alt+Shift+XF86MonBrightnessDown"),
        params: no_params,
    },
    CommandSpec {
        id: SYSTEM_DND,
        label: "Non disturbare",
        description: "Con Non disturbare attivo le notifiche arrivano nel Centro senza banner.",
        shortcut: None,
        params: enabled_param,
    },
    CommandSpec {
        id: GLASS_LIST,
        label: "Elenca il vetro",
        description: "Superfici con regioni di vetro (silica-glass-v1): superficie, origine e regioni.",
        shortcut: None,
        params: no_params,
    },
    CommandSpec {
        id: WINDOW_FOCUS,
        label: "Porta in primo piano",
        description: "Dà il focus a una finestra e la porta sopra le altre; se è ridotta la riporta.",
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
        id: APP_QUIT,
        label: "Esci dall'app",
        description: "Chiede a tutte le finestre dell'app in primo piano (stesso app_id) di chiudersi.",
        shortcut: Some("Logo+Q"),
        params: no_params,
    },
    CommandSpec {
        id: WINDOW_MINIMIZE,
        label: "Riduci finestra",
        description: "Nasconde la finestra; window.focus la riporta.",
        shortcut: Some("Logo+M"),
        params: window_target,
    },
    CommandSpec {
        id: WINDOW_FULLSCREEN,
        label: "Schermo intero",
        description: "Porta la finestra a schermo intero, o la riporta com'era.",
        shortcut: Some("Logo+Ctrl+F"),
        params: window_target,
    },
    CommandSpec {
        id: WINDOW_TILE_LEFT,
        label: "Metà sinistra",
        description: "Porta la finestra nella metà sinistra dello schermo.",
        shortcut: None,
        params: window_target,
    },
    CommandSpec {
        id: WINDOW_TILE_RIGHT,
        label: "Metà destra",
        description: "Porta la finestra nella metà destra dello schermo.",
        shortcut: None,
        params: window_target,
    },
    CommandSpec {
        id: WINDOW_TILE_TOP,
        label: "Metà alta",
        description: "Porta la finestra nella metà alta dello schermo.",
        shortcut: None,
        params: window_target,
    },
    CommandSpec {
        id: WINDOW_TILE_BOTTOM,
        label: "Metà bassa",
        description: "Porta la finestra nella metà bassa dello schermo.",
        shortcut: None,
        params: window_target,
    },
    CommandSpec {
        id: WINDOW_TILE_FILL,
        label: "Riempi",
        description: "La finestra riempie lo schermo, senza schermo intero.",
        shortcut: None,
        params: window_target,
    },
    CommandSpec {
        id: WINDOW_ARRANGE_2,
        label: "Disponi due finestre",
        description: "La finestra a sinistra e l'ultima usata a destra.",
        shortcut: None,
        params: window_target,
    },
    CommandSpec {
        id: WINDOW_ARRANGE_3,
        label: "Disponi tre finestre",
        description: "La finestra a sinistra e le due ultime usate impilate a destra.",
        shortcut: None,
        params: window_target,
    },
    CommandSpec {
        id: WINDOW_ARRANGE_4,
        label: "Disponi quattro finestre",
        description: "La finestra e le tre ultime usate nei quarti dello schermo.",
        shortcut: None,
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
                "properties": {
                    "command": {"type": "string"},
                    "shell": {"type": "boolean", "description": "Componente della shell (vede silica-shell-v1)"}
                },
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
        id: INPUT_GESTURE,
        label: "Gesto del trackpad",
        description: "Un gesto a più dita: `kind` swipe o pinch, `fingers`, `phase` begin, update o end (senza phase il gesto intero in una chiamata), `dx` e `dy` (scorrimento, punti), `scale` (pizzico, assoluta), `cancelled`. Alla fine restituisce `command`, il comando eseguito: tre o quattro dita a sinistra o a destra cambiano spazio, in alto Panorama, in basso le finestre dell'app; pollice e tre dita allargando la scrivania, stringendo il launcher.",
        shortcut: None,
        params: || {
            json!({
                "type": "object",
                "properties": {
                    "kind": {"enum": ["swipe", "pinch"]},
                    "fingers": {"type": "integer", "minimum": 1, "maximum": 10},
                    "phase": {"enum": ["begin", "update", "end"]},
                    "dx": {"type": "number"},
                    "dy": {"type": "number"},
                    "scale": {"type": "number"},
                    "cancelled": {"type": "boolean"}
                },
                "required": ["kind", "fingers"],
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
    CommandSpec {
        id: PERF_FRAMES,
        label: "Misura i frame",
        description: "Per ogni superficie con buffer: frame ricevuti, intervalli fra i commit (media, p50, p95, p99, massimo), frame lenti (oltre 1,5 periodi di refresh) e tempo dall'avvio al primo buffer. Con reset azzera le misure dopo averle restituite.",
        shortcut: None,
        params: || {
            json!({
                "type": "object",
                "properties": {"reset": {"type": "boolean"}},
                "additionalProperties": false
            })
        },
    },
    CommandSpec {
        id: PERF_PROCESS,
        label: "Misura un processo",
        description: "Memoria di un processo client (RSS e PSS da /proc, in KiB).",
        shortcut: None,
        params: || {
            json!({
                "type": "object",
                "properties": {"pid": {"type": "integer", "minimum": 1}},
                "required": ["pid"],
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
    /// Un componente della shell: vede i protocolli della shell.
    #[serde(default)]
    pub shell: bool,
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

/// Parametri di [`PERF_FRAMES`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PerfFramesParams {
    #[serde(default)]
    pub reset: bool,
}

/// Le misure di una superficie in [`PERF_FRAMES`]. Tempi in millisecondi.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SurfaceFrames {
    /// `window <id>`, `layer <namespace>` o `surface` per le altre.
    pub surface: String,
    pub pid: Option<u32>,
    /// Commit con un buffer nuovo dall'ultimo reset.
    pub frames: u64,
    pub mean_ms: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub max_ms: f64,
    /// Intervalli oltre 1,5 periodi di refresh dell'output.
    pub slow: u64,
    pub period_ms: f64,
    /// Dal lancio con `app.launch` (o `--spawn`) al primo buffer, se lanciato da Ossido.
    pub startup_ms: Option<f64>,
}

/// Parametri di [`PERF_PROCESS`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PerfProcessParams {
    pub pid: u32,
}

/// Il risultato di [`PERF_PROCESS`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub rss_kib: u64,
    pub pss_kib: u64,
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
    /// Ridotta: non è sullo schermo, window.focus la riporta.
    #[serde(default)]
    pub minimized: bool,
    /// A schermo intero.
    #[serde(default)]
    pub fullscreen: bool,
    /// Lo spazio di lavoro della finestra (da 0).
    #[serde(default)]
    pub space: usize,
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

/// Parametri di [`UI_ANIMATION_SCALE`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnimationScale {
    pub scale: f64,
}

/// Parametri di [`SPACE_GOTO`] e [`SPACE_REMOVE`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndexParams {
    pub index: usize,
}

/// Parametri di [`WINDOW_MOVE_TO_SPACE`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MoveToSpaceParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<u64>,
    pub index: usize,
}

/// Una finestra disposta in [`PANORAMA_STATE`] (punti logici).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PanoramaWindow {
    pub id: u64,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Un rettangolo in punti logici.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PanoramaRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Risposta di [`PANORAMA_STATE`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PanoramaState {
    /// `panorama`, `app` (App Exposé) o `desktop` (Mostra scrivania); assente se chiuso.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    pub open: bool,
    pub progress: f64,
    pub windows: Vec<PanoramaWindow>,
    /// Espansione della barra degli spazi (0 compatta, 1 espansa).
    pub expansion: f64,
    /// Le miniature degli spazi, quando si vedono.
    pub thumbnails: Vec<PanoramaRect>,
}

/// Parametri di [`INPUT_GESTURE`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GestureParams {
    pub kind: String,
    pub fingers: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
    #[serde(default)]
    pub dx: f64,
    #[serde(default)]
    pub dy: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<f64>,
    #[serde(default)]
    pub cancelled: bool,
}

/// Uno spazio in [`SPACE_LIST`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpaceInfo {
    pub index: usize,
    pub active: bool,
    /// Id delle finestre, dal basso verso l'alto.
    pub windows: Vec<u64>,
}

/// Parametri di [`SYSTEM_WIFI`], [`SYSTEM_BLUETOOTH`], [`SYSTEM_MUTE`], [`SYSTEM_DND`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnabledParams {
    pub enabled: bool,
}

/// Parametri di [`SYSTEM_VOLUME`] e [`SYSTEM_BRIGHTNESS`], da 0 a 1.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValueParams {
    pub value: f64,
}

/// Parametri di [`UI_GLASS_QUALITY`]: `auto`, `full`, `frosted`, `opaque`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GlassQualityParams {
    pub quality: String,
}

/// Risultato di [`PERF_RENDER`]: il tempo del rendering di Ossido per frame, in ms.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RenderTimes {
    pub frames: u64,
    pub mean_ms: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub max_ms: f64,
}

/// Parametri di [`PERF_RENDER`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PerfRenderParams {
    #[serde(default)]
    pub reset: bool,
}
