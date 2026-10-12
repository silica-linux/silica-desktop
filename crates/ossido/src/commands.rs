//! Esecuzione dei comandi del catalogo (`silica_control::commands`).

use std::process::Command;

use base64::Engine;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use silica_control::commands::{
    self, APP_LAUNCH, AppLaunch, Click, ENGINE_COMMANDS, ENGINE_EXECUTE, ExecuteParams,
    INPUT_CLICK, INPUT_KEY, INPUT_POINTER_BUTTON, INPUT_POINTER_MOVE, INPUT_TYPE, Key, LAYER_LIST,
    PointerButton, PointerMove, SESSION_QUIT, TypeText, UI_SCREENSHOT, WINDOW_CLOSE, WINDOW_FOCUS,
    WINDOW_LIST, WINDOW_MOVE, WindowInfo, WindowMove, WindowTarget,
};
use silica_control::{Request, Response, Screenshot, ScreenshotParams, Shortcut};
use smithay::backend::input::{ButtonState, KeyState};
use smithay::desktop::Window;
use smithay::input::keyboard::{FilterResult, Keycode};
use smithay::utils::SERIAL_COUNTER;
use smithay::wayland::compositor::with_states;
use smithay::wayland::shell::xdg::XdgToplevelSurfaceData;
use tracing::{info, warn};

use crate::state::State;

/// Id stabile di una finestra, salvato nei suoi user data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowId(pub u64);

/// Le scorciatoie del catalogo, già lette.
pub fn shortcuts() -> Vec<(Shortcut, &'static str)> {
    commands::CATALOG
        .iter()
        .filter_map(|spec| {
            let text = spec.shortcut?;
            match Shortcut::parse(text) {
                Ok(shortcut) => Some((shortcut, spec.id)),
                Err(err) => {
                    warn!("scorciatoia di {} ignorata: {err}", spec.id);
                    None
                }
            }
        })
        .collect()
}

fn parse<T: DeserializeOwned + Default>(params: Value) -> Result<T, String> {
    if params.is_null() {
        return Ok(T::default());
    }
    serde_json::from_value(params).map_err(|e| format!("parametri: {e}"))
}

fn parse_required<T: DeserializeOwned>(params: Value) -> Result<T, String> {
    serde_json::from_value(params).map_err(|e| format!("parametri: {e}"))
}

fn to_value<T: serde::Serialize>(value: T) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|e| e.to_string())
}

impl State {
    /// Esegue una richiesta del canale di controllo.
    pub fn execute(&mut self, request: Request) -> Response {
        match self.execute_command(&request.command, request.params) {
            Ok(value) => Response::ok(request.id, value),
            Err(err) => Response::err(request.id, err),
        }
    }

    /// Esegue un comando del catalogo: lo stesso punto d'ingresso per canale,
    /// scorciatoie e, più avanti, UI e MCP.
    pub fn execute_command(&mut self, id: &str, params: Value) -> Result<Value, String> {
        match id {
            UI_SCREENSHOT => self.command_screenshot(parse(params)?),
            ENGINE_COMMANDS => Ok(commands::catalog_json()),
            ENGINE_EXECUTE => {
                let ExecuteParams { id, params } = parse_required(params)?;
                if id == ENGINE_EXECUTE {
                    return Err("engine.execute non si annida".to_owned());
                }
                self.execute_command(&id, params)
            }
            WINDOW_LIST => to_value(self.window_list()),
            LAYER_LIST => to_value(self.layer_list()),
            WINDOW_FOCUS => {
                let target: WindowTarget = parse(params)?;
                let window = self.target_window(target.id)?;
                self.focus_window(Some(window));
                Ok(Value::Null)
            }
            WINDOW_CLOSE => {
                let target: WindowTarget = parse(params)?;
                let window = self.target_window(target.id)?;
                let toplevel = window.toplevel().ok_or("finestra senza toplevel")?;
                toplevel.send_close();
                Ok(Value::Null)
            }
            WINDOW_MOVE => {
                let WindowMove { id, x, y } = parse_required(params)?;
                let window = self.target_window(id)?;
                self.space.map_element(window, (x, y), false);
                Ok(Value::Null)
            }
            APP_LAUNCH => {
                let AppLaunch { command } = parse_required(params)?;
                let pid = self.launch(&command)?;
                Ok(json!({ "pid": pid }))
            }
            INPUT_POINTER_MOVE => {
                let PointerMove { x, y } = parse_required(params)?;
                self.pointer_move_to((x, y).into(), self.now_msec());
                Ok(Value::Null)
            }
            INPUT_POINTER_BUTTON => {
                let PointerButton { button, pressed } = parse_required(params)?;
                let state = if pressed {
                    ButtonState::Pressed
                } else {
                    ButtonState::Released
                };
                self.pointer_button(button.code(), state, self.now_msec());
                Ok(Value::Null)
            }
            INPUT_CLICK => {
                let Click { x, y, button } = parse_required(params)?;
                let time = self.now_msec();
                self.pointer_move_to((x, y).into(), time);
                self.pointer_button(button.code(), ButtonState::Pressed, time);
                self.pointer_button(button.code(), ButtonState::Released, time);
                Ok(Value::Null)
            }
            INPUT_KEY => {
                let Key { key, pressed } = parse_required(params)?;
                let (code, shift) = self
                    .keys
                    .find(&key)
                    .ok_or_else(|| format!("tasto sconosciuto: {key}"))?;
                let states: &[KeyState] = match pressed {
                    Some(true) => &[KeyState::Pressed],
                    Some(false) => &[KeyState::Released],
                    None => &[KeyState::Pressed, KeyState::Released],
                };
                for state in states {
                    self.synthetic_key(code, shift, *state);
                }
                Ok(Value::Null)
            }
            INPUT_TYPE => {
                let TypeText { text } = parse_required(params)?;
                // Si controlla tutto prima di premere qualcosa: niente testo a metà.
                let codes = text
                    .chars()
                    .map(|ch| {
                        self.keys
                            .find(&ch.to_string())
                            .ok_or_else(|| format!("carattere non digitabile: {ch:?}"))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                for (code, shift) in codes {
                    self.synthetic_key(code, shift, KeyState::Pressed);
                    self.synthetic_key(code, shift, KeyState::Released);
                }
                Ok(Value::Null)
            }
            SESSION_QUIT => {
                info!("uscita richiesta");
                self.loop_signal.stop();
                Ok(Value::Null)
            }
            other => Err(format!("comando sconosciuto: {other}")),
        }
    }

    /// Un tasto sintetico verso la finestra con il focus, con Maiusc attorno se serve.
    /// Passa dallo stesso filtro dei tasti veri, quindi anche le scorciatoie funzionano.
    fn synthetic_key(&mut self, code: u32, shift: bool, state: KeyState) {
        let Some(keyboard) = self.seat.get_keyboard() else {
            return;
        };
        let time = self.now_msec();
        let press = |state: &mut Self, code: u32, key_state: KeyState| {
            keyboard.input::<(), _>(
                state,
                Keycode::new(code),
                key_state,
                SERIAL_COUNTER.next_serial(),
                time,
                |_, _, _| FilterResult::Forward,
            );
        };
        if shift && state == KeyState::Pressed {
            press(self, crate::keys::SHIFT_LEFT, KeyState::Pressed);
        }
        press(self, code, state);
        if shift && state == KeyState::Released {
            press(self, crate::keys::SHIFT_LEFT, KeyState::Released);
        }
    }

    /// Tempo per gli eventi di input sintetici, in millisecondi dall'avvio.
    fn now_msec(&self) -> u32 {
        u32::try_from(self.start_time.elapsed().as_millis()).unwrap_or(u32::MAX)
    }

    /// Avvia un comando di shell come client di questa sessione.
    pub fn launch(&self, command: &str) -> Result<u32, String> {
        let child = Command::new("sh")
            .arg("-c")
            .arg(command)
            .env("WAYLAND_DISPLAY", &self.socket_name)
            .spawn()
            .map_err(|e| format!("avvio di {command}: {e}"))?;
        info!("avviato {command} (pid {})", child.id());
        Ok(child.id())
    }

    pub fn assign_window_id(&mut self, window: &Window) -> u64 {
        self.next_window_id = self.next_window_id.wrapping_add(1);
        let id = self.next_window_id;
        window.user_data().insert_if_missing(|| WindowId(id));
        id
    }

    fn window_id(window: &Window) -> Option<u64> {
        window.user_data().get::<WindowId>().map(|id| id.0)
    }

    /// La finestra con quell'id, oppure quella con il focus.
    fn target_window(&self, id: Option<u64>) -> Result<Window, String> {
        match id {
            Some(id) => self
                .space
                .elements()
                .find(|w| Self::window_id(w) == Some(id))
                .cloned()
                .ok_or_else(|| format!("nessuna finestra con id {id}")),
            None => self
                .focused_window()
                .ok_or_else(|| "nessuna finestra ha il focus".to_owned()),
        }
    }

    fn focused_window(&self) -> Option<Window> {
        let focus = self.seat.get_keyboard()?.current_focus()?;
        self.space
            .elements()
            .find(|w| w.toplevel().is_some_and(|t| t.wl_surface() == &focus))
            .cloned()
    }

    /// Dà il focus a una finestra (o a nessuna) e la porta in cima.
    pub fn focus_window(&mut self, window: Option<Window>) {
        for other in self.space.elements() {
            other.set_activated(false);
        }
        let focus = window
            .as_ref()
            .and_then(|w| w.toplevel().map(|t| t.wl_surface().clone()));
        if let Some(window) = &window {
            self.space.raise_element(window, true);
        }
        // Un layer con tastiera esclusiva sopra le finestre tiene il focus (wlr-layer-shell).
        let focus = self.exclusive_layer_surface().or(focus);
        if let Some(keyboard) = self.seat.get_keyboard() {
            keyboard.set_focus(self, focus, SERIAL_COUNTER.next_serial());
        }
        for window in self.space.elements() {
            if let Some(toplevel) = window.toplevel() {
                toplevel.send_pending_configure();
            }
        }
    }

    fn window_list(&self) -> Vec<WindowInfo> {
        let focused = self.focused_window();
        // Space elenca dal basso verso l'alto: si rovescia per avere la cima prima.
        let windows: Vec<&Window> = self.space.elements().collect();
        windows
            .into_iter()
            .rev()
            .filter_map(|window| {
                let id = Self::window_id(window)?;
                let location = self.space.element_location(window).unwrap_or_default();
                let size = window.geometry().size;
                let (title, app_id) = window
                    .toplevel()
                    .map(|toplevel| {
                        with_states(toplevel.wl_surface(), |states| {
                            states
                                .data_map
                                .get::<XdgToplevelSurfaceData>()
                                .and_then(|data| data.lock().ok())
                                .map(|data| (data.title.clone(), data.app_id.clone()))
                                .unwrap_or_default()
                        })
                    })
                    .unwrap_or_default();
                Some(WindowInfo {
                    id,
                    title,
                    app_id,
                    x: location.x,
                    y: location.y,
                    width: size.w,
                    height: size.h,
                    focused: focused.as_ref() == Some(window),
                })
            })
            .collect()
    }

    fn command_screenshot(&mut self, params: ScreenshotParams) -> Result<Value, String> {
        let capture = self.screenshot()?;
        let png = capture.to_png()?;
        let mut shot = Screenshot {
            width: capture.width,
            height: capture.height,
            path: None,
            png_base64: None,
        };
        match params.path {
            Some(path) => {
                std::fs::write(&path, &png).map_err(|e| format!("scrittura di {path}: {e}"))?;
                shot.path = Some(path);
            }
            None => shot.png_base64 = Some(base64::engine::general_purpose::STANDARD.encode(&png)),
        }
        to_value(shot)
    }
}
