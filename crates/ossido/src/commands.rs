//! Esecuzione dei comandi del catalogo (`silica_control::commands`).

use std::process::Command;

use base64::Engine;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use silica_control::commands::{
    self, APP_LAUNCH, AnimationScale, AppLaunch, Click, ENGINE_COMMANDS, ENGINE_EXECUTE,
    ExecuteParams, FARO_TOGGLE, GLASS_LIST, GlassQualityParams, INPUT_CLICK, INPUT_KEY,
    INPUT_POINTER_BUTTON, INPUT_POINTER_MOVE, INPUT_TYPE, Key, LAYER_LIST, NOTIFICATIONS_TOGGLE,
    PERF_FRAMES, PERF_PROCESS, PERF_RENDER, PerfFramesParams, PerfProcessParams, PerfRenderParams,
    PointerButton, PointerMove, SESSION_QUIT, TypeText, UI_ANIMATION_SCALE, UI_GLASS_QUALITY,
    UI_SCREENSHOT, WINDOW_ARRANGE_2, WINDOW_ARRANGE_3, WINDOW_ARRANGE_4, WINDOW_CLOSE,
    WINDOW_FOCUS, WINDOW_FULLSCREEN, WINDOW_LIST, WINDOW_MINIMIZE, WINDOW_MOVE, WINDOW_TILE_BOTTOM,
    WINDOW_TILE_FILL, WINDOW_TILE_LEFT, WINDOW_TILE_RIGHT, WINDOW_TILE_TOP, WindowInfo, WindowMove,
    WindowTarget,
};
use silica_control::commands::{
    APP_FORCE_QUIT, APP_FORCE_QUIT_PANEL, SCREENSHOT_REGION, SCREENSHOT_SCREEN,
    SCREENSHOT_SELECTION, SCREENSHOT_TOOLBAR, SCREENSHOT_WINDOW_AT,
};
use silica_control::commands::{APP_QUIT, GestureParams, INPUT_GESTURE};
use silica_control::commands::{
    CONTROLCENTER_TOGGLE, EnabledParams, SYSTEM_BLUETOOTH, SYSTEM_BRIGHTNESS,
    SYSTEM_BRIGHTNESS_DOWN, SYSTEM_BRIGHTNESS_DOWN_FINE, SYSTEM_BRIGHTNESS_UP,
    SYSTEM_BRIGHTNESS_UP_FINE, SYSTEM_DND, SYSTEM_MUTE, SYSTEM_MUTE_TOGGLE, SYSTEM_STATUS,
    SYSTEM_VOLUME, SYSTEM_VOLUME_DOWN, SYSTEM_VOLUME_DOWN_FINE, SYSTEM_VOLUME_UP,
    SYSTEM_VOLUME_UP_FINE, SYSTEM_WIFI, ValueParams,
};
use silica_control::commands::{
    DESKTOP_SHOW, PANORAMA_APP, PANORAMA_SPACES_BAR, PANORAMA_STATE, PANORAMA_TOGGLE,
};
use silica_control::commands::{
    IndexParams, MoveToSpaceParams, SPACE_ADD, SPACE_GOTO, SPACE_LIST, SPACE_NEXT, SPACE_PREVIOUS,
    SPACE_REMOVE, SpaceInfo, WINDOW_MOVE_TO_SPACE,
};
use silica_control::commands::{
    LAUNCHER_TOGGLE, SWITCHER_CANCEL, SWITCHER_COMMIT, SWITCHER_NEXT, SWITCHER_PREVIOUS,
};
use silica_control::{Request, Response, Screenshot, ScreenshotParams, Shortcut};
use smithay::backend::input::{ButtonState, KeyState};
use smithay::desktop::Window;
use smithay::input::keyboard::Keycode;
use smithay::reexports::wayland_server::Resource;
use smithay::utils::{Logical, Point, SERIAL_COUNTER};
use smithay::wayland::compositor::with_states;
use smithay::wayland::shell::xdg::XdgToplevelSurfaceData;
use tracing::{info, warn};

use crate::state::State;
use crate::system::Call;

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
            SCREENSHOT_SCREEN => {
                let capture = self.screenshot()?;
                self.publish_screenshot(&capture)
            }
            SCREENSHOT_REGION => {
                let region: silica_control::RegionParams = parse_required(params)?;
                let capture = self.screenshot()?;
                let scale = self.output.current_scale().fractional_scale();
                let origin = self
                    .space
                    .output_geometry(&self.output)
                    .map(|g| g.loc.to_f64())
                    .unwrap_or_default();
                let crop = crate::capture::crop(
                    &capture,
                    (region.x - origin.x) * scale,
                    (region.y - origin.y) * scale,
                    region.width * scale,
                    region.height * scale,
                )
                .ok_or("regione vuota o fuori dallo schermo")?;
                self.publish_screenshot(&crop)
            }
            SCREENSHOT_WINDOW_AT => {
                let point: silica_control::PointParams = parse_required(params)?;
                let window = match self.window_hit(Point::from((point.x, point.y))) {
                    Some(crate::decorations::Hit::Titlebar(window, _, _))
                    | Some(crate::decorations::Hit::Content(window)) => window,
                    None => return Err("nessuna finestra in quel punto".to_owned()),
                };
                let capture = self.window_capture(&window)?;
                self.publish_screenshot(&capture)
            }
            APP_FORCE_QUIT => {
                let silica_control::AppParams { app_id } = parse_required(params)?;
                let killed = self.force_quit(&app_id)?;
                Ok(json!({ "killed": killed }))
            }
            SCREENSHOT_SELECTION | SCREENSHOT_TOOLBAR | APP_FORCE_QUIT_PANEL => {
                Ok(json!({ "forwarded": self.forward_to_shell(id) }))
            }
            UI_GLASS_QUALITY => {
                let GlassQualityParams { quality } = parse_required(params)?;
                self.glass_setting =
                    crate::state::GlassSetting::parse(&quality).ok_or_else(|| {
                        format!(
                            "qualità del vetro sconosciuta (auto, full, frosted, opaque): {quality}"
                        )
                    })?;
                Ok(json!({ "quality": self.glass_quality().name() }))
            }
            PERF_RENDER => {
                let PerfRenderParams { reset } = parse(params)?;
                to_value(self.perf.render_times(reset))
            }
            UI_ANIMATION_SCALE => {
                let AnimationScale { scale } = parse_required(params)?;
                if !scale.is_finite() || !(0.0..=1000.0).contains(&scale) {
                    return Err(format!(
                        "scala delle animazioni non valida (0–1000): {scale}"
                    ));
                }
                self.animation_speed = scale;
                Ok(Value::Null)
            }
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
            GLASS_LIST => to_value(self.glass_list()),
            // Comandi dei componenti della shell: Ossido li inoltra.
            SPACE_LIST => to_value(self.space_list()),
            SPACE_NEXT | SPACE_PREVIOUS => {
                let active = self.workspaces.active;
                let to = if id == SPACE_NEXT {
                    active.saturating_add(1)
                } else {
                    // Prima del primo spazio non si fa niente (nessun rimbalzo).
                    match active.checked_sub(1) {
                        Some(to) => to,
                        None => return Ok(json!({ "switched": false })),
                    }
                };
                Ok(json!({ "switched": self.switch_space(to, std::time::Instant::now()) }))
            }
            SPACE_GOTO => {
                let IndexParams { index } = parse_required(params)?;
                if index >= self.workspaces.count() {
                    return Err(format!("spazio {index} assente"));
                }
                Ok(json!({ "switched": self.switch_space(index, std::time::Instant::now()) }))
            }
            PANORAMA_TOGGLE => {
                self.toggle_panorama(std::time::Instant::now());
                Ok(json!({ "open": self.panorama_open() }))
            }
            PANORAMA_STATE => to_value(self.panorama_state()),
            PANORAMA_APP | DESKTOP_SHOW => {
                let mode = if id == PANORAMA_APP {
                    crate::panorama::Mode::App
                } else {
                    crate::panorama::Mode::Desktop
                };
                self.toggle_view(mode, std::time::Instant::now());
                Ok(json!({ "open": self.panorama_mode() == Some(mode) }))
            }
            PANORAMA_SPACES_BAR => {
                let EnabledParams { enabled } = parse_required(params)?;
                if !self.panorama_open() {
                    return Err("Panorama non è aperto".to_owned());
                }
                self.expand_spaces_bar(enabled, std::time::Instant::now());
                Ok(Value::Null)
            }
            SPACE_ADD => Ok(json!({ "index": self.add_space()? })),
            SPACE_REMOVE => {
                let IndexParams { index } = parse_required(params)?;
                self.remove_space(index)?;
                Ok(Value::Null)
            }
            WINDOW_MOVE_TO_SPACE => {
                let MoveToSpaceParams { id, index } = parse_required(params)?;
                let window = match id.and_then(|id| self.stored_window(id)) {
                    Some(window) => window,
                    None => self.target_window(id)?,
                };
                self.move_window_to_space(&window, index)?;
                Ok(Value::Null)
            }
            SYSTEM_STATUS => self
                .system
                .status()
                .ok_or_else(|| "silica-daemon non raggiungibile".to_owned()),
            SYSTEM_WIFI | SYSTEM_BLUETOOTH | SYSTEM_MUTE | SYSTEM_DND => {
                let EnabledParams { enabled } = parse_required(params)?;
                let call = match id {
                    SYSTEM_WIFI => Call::Wifi(enabled),
                    SYSTEM_BLUETOOTH => Call::Bluetooth(enabled),
                    SYSTEM_MUTE => Call::Muted(enabled),
                    _ => Call::DoNotDisturb(enabled),
                };
                Ok(json!({ "queued": self.system.queue(call) }))
            }
            SYSTEM_VOLUME | SYSTEM_BRIGHTNESS => {
                let ValueParams { value } = parse_required(params)?;
                if !value.is_finite() {
                    return Err("valore non finito".to_owned());
                }
                let value = value.clamp(0.0, 1.0);
                let call = if id == SYSTEM_VOLUME {
                    Call::Volume(value)
                } else {
                    Call::Brightness(value)
                };
                Ok(json!({ "queued": self.system.queue(call) }))
            }
            SYSTEM_VOLUME_UP
            | SYSTEM_VOLUME_DOWN
            | SYSTEM_VOLUME_UP_FINE
            | SYSTEM_VOLUME_DOWN_FINE => Ok(json!({
                "queued": self.system.volume_step(
                    matches!(id, SYSTEM_VOLUME_UP | SYSTEM_VOLUME_UP_FINE),
                    matches!(id, SYSTEM_VOLUME_UP_FINE | SYSTEM_VOLUME_DOWN_FINE),
                )
            })),
            SYSTEM_MUTE_TOGGLE => Ok(json!({ "queued": self.system.mute_toggle() })),
            SYSTEM_BRIGHTNESS_UP
            | SYSTEM_BRIGHTNESS_DOWN
            | SYSTEM_BRIGHTNESS_UP_FINE
            | SYSTEM_BRIGHTNESS_DOWN_FINE => Ok(json!({
                "queued": self.system.brightness_step(
                    matches!(id, SYSTEM_BRIGHTNESS_UP | SYSTEM_BRIGHTNESS_UP_FINE),
                    matches!(id, SYSTEM_BRIGHTNESS_UP_FINE | SYSTEM_BRIGHTNESS_DOWN_FINE),
                )
            })),
            SWITCHER_NEXT | SWITCHER_PREVIOUS => {
                self.switcher_active = true;
                Ok(json!({ "forwarded": self.forward_to_shell(id) }))
            }
            SWITCHER_COMMIT | SWITCHER_CANCEL => {
                self.switcher_active = false;
                Ok(json!({ "forwarded": self.forward_to_shell(id) }))
            }
            FARO_TOGGLE | NOTIFICATIONS_TOGGLE | CONTROLCENTER_TOGGLE | LAUNCHER_TOGGLE => {
                let sent = self.forward_to_shell(id);
                Ok(json!({ "forwarded": sent }))
            }
            WINDOW_FOCUS => {
                let target: WindowTarget = parse(params)?;
                // Una finestra di un altro spazio: si passa a quello spazio.
                if let Some(id) = target.id
                    && let Some(space) = self.stored_window_space(id)
                {
                    self.switch_space(space, std::time::Instant::now());
                }
                let window = match target.id.and_then(|id| self.minimized_window(id)) {
                    Some(window) => {
                        self.restore_window(&window);
                        window
                    }
                    None => self.target_window(target.id)?,
                };
                self.focus_window(Some(window));
                Ok(Value::Null)
            }
            WINDOW_MINIMIZE => {
                let target: WindowTarget = parse(params)?;
                let window = self.target_window(target.id)?;
                self.minimize_window(&window)?;
                Ok(Value::Null)
            }
            WINDOW_FULLSCREEN => {
                let target: WindowTarget = parse(params)?;
                let window = self.target_window(target.id)?;
                self.toggle_fullscreen(&window)?;
                Ok(Value::Null)
            }
            WINDOW_TILE_LEFT | WINDOW_TILE_RIGHT | WINDOW_TILE_TOP | WINDOW_TILE_BOTTOM
            | WINDOW_TILE_FILL => {
                let target: WindowTarget = parse(params)?;
                let window = self.target_window(target.id)?;
                let tile = match id {
                    WINDOW_TILE_LEFT => crate::windows::Tile::Left,
                    WINDOW_TILE_RIGHT => crate::windows::Tile::Right,
                    WINDOW_TILE_TOP => crate::windows::Tile::Top,
                    WINDOW_TILE_BOTTOM => crate::windows::Tile::Bottom,
                    _ => crate::windows::Tile::Fill,
                };
                self.tile_window(&window, tile)?;
                Ok(Value::Null)
            }
            WINDOW_ARRANGE_2 | WINDOW_ARRANGE_3 | WINDOW_ARRANGE_4 => {
                let target: WindowTarget = parse(params)?;
                let window = self.target_window(target.id)?;
                let count = match id {
                    WINDOW_ARRANGE_2 => 2,
                    WINDOW_ARRANGE_3 => 3,
                    _ => 4,
                };
                self.arrange_windows(&window, count)?;
                Ok(Value::Null)
            }
            WINDOW_CLOSE => {
                let target: WindowTarget = parse(params)?;
                let window = self.target_window(target.id)?;
                let toplevel = window.toplevel().ok_or("finestra senza toplevel")?;
                toplevel.send_close();
                Ok(Value::Null)
            }
            APP_QUIT => {
                let window = self.target_window(None)?;
                let app = crate::panorama::window_app(&window);
                let mut windows: Vec<Window> = vec![window];
                if let Some(app) = app {
                    windows = self
                        .space
                        .elements()
                        .chain(self.minimized.iter().map(|(w, _)| w))
                        .chain(self.workspaces.stored.iter().flatten().map(|(w, _)| w))
                        .filter(|w| crate::panorama::window_app(w).as_deref() == Some(app.as_str()))
                        .cloned()
                        .collect();
                }
                for window in &windows {
                    if let Some(toplevel) = window.toplevel() {
                        toplevel.send_close();
                    }
                }
                Ok(json!({ "closed": windows.len() }))
            }
            WINDOW_MOVE => {
                let WindowMove { id, x, y } = parse_required(params)?;
                let window = self.target_window(id)?;
                self.space.map_element(window, (x, y), false);
                Ok(Value::Null)
            }
            APP_LAUNCH => {
                let AppLaunch { command, shell } = parse_required(params)?;
                let pid = self.launch_with(&command, shell)?;
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
            INPUT_GESTURE => {
                let p: GestureParams = parse_required(params)?;
                let kind = match p.kind.as_str() {
                    "swipe" => crate::gestures::GestureKind::Swipe,
                    "pinch" => crate::gestures::GestureKind::Pinch,
                    other => return Err(format!("gesto sconosciuto: {other}")),
                };
                let phase = p.phase.as_deref();
                if matches!(phase, None | Some("begin")) {
                    self.gesture_begin(kind, p.fingers);
                }
                match phase {
                    None | Some("update") => self.gesture_update(p.dx, p.dy, p.scale),
                    Some("begin" | "end") => {}
                    Some(other) => return Err(format!("fase sconosciuta: {other}")),
                }
                if matches!(phase, None | Some("end")) {
                    let command = self.gesture_end(p.cancelled);
                    return Ok(json!({ "command": command }));
                }
                Ok(json!({ "command": null }))
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
            PERF_FRAMES => {
                let PerfFramesParams { reset } = parse(params)?;
                let report = self.perf_frames();
                if reset {
                    self.perf.reset();
                }
                to_value(report)
            }
            PERF_PROCESS => {
                let PerfProcessParams { pid } = parse_required(params)?;
                to_value(crate::perf::process_info(pid)?)
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
        let time = self.now_msec();
        // Come un tasto vero: anche le scorciatoie del catalogo.
        let press = |state: &mut Self, code: u32, key_state: KeyState| {
            state.handle_key(Keycode::new(code), key_state, time);
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
    pub fn launch(&mut self, command: &str) -> Result<u32, String> {
        self.launch_with(command, false)
    }

    /// Come `launch`; con `shell` il processo è un componente della shell e vede i suoi
    /// protocolli (`shell.rs`).
    pub fn launch_with(&mut self, command: &str, shell: bool) -> Result<u32, String> {
        let mut process = Command::new("sh");
        process
            .arg("-c")
            .arg(command)
            .env("WAYLAND_DISPLAY", &self.socket_name);
        if let Some(address) = &self.session_bus {
            process.env("DBUS_SESSION_BUS_ADDRESS", address);
        }
        if shell {
            process.env(silica_protocols::shell::SHELL_ENV, "1");
        } else {
            process.env_remove(silica_protocols::shell::SHELL_ENV);
        }
        let child = process
            .spawn()
            .map_err(|e| format!("avvio di {command}: {e}"))?;
        info!("avviato {command} (pid {})", child.id());
        self.perf.launched(child.id(), std::time::Instant::now());
        Ok(child.id())
    }

    /// Le misure dei frame, con le superfici chiamate come in `window.list` e `layer.list`.
    fn perf_frames(&mut self) -> Vec<silica_control::commands::SurfaceFrames> {
        let mut names = std::collections::HashMap::new();
        for window in self.space.elements() {
            if let (Some(toplevel), Some(id)) = (window.toplevel(), Self::window_id(window)) {
                names.insert(toplevel.wl_surface().id(), format!("window {id}"));
            }
        }
        for layer in smithay::desktop::layer_map_for_output(&self.output).layers() {
            names.insert(
                layer.wl_surface().id(),
                format!("layer {}", layer.namespace()),
            );
        }
        let refresh = self
            .output
            .current_mode()
            .map_or(0, |mode| mode.refresh)
            .max(1);
        // refresh è in mHz; senza un valore sensato si assume 60 Hz.
        let period = if refresh < 1000 {
            std::time::Duration::from_micros(16_667)
        } else {
            std::time::Duration::from_secs_f64(1000.0 / f64::from(refresh))
        };
        self.perf.report(period, |id| {
            names
                .get(id)
                .cloned()
                .unwrap_or_else(|| "surface".to_owned())
        })
    }

    pub fn assign_window_id(&mut self, window: &Window) -> u64 {
        self.next_window_id = self.next_window_id.wrapping_add(1);
        let id = self.next_window_id;
        window.user_data().insert_if_missing(|| WindowId(id));
        id
    }

    pub fn window_id(window: &Window) -> Option<u64> {
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

    /// Gli spazi con le loro finestre (`space.list`).
    fn space_list(&self) -> Vec<SpaceInfo> {
        (0..self.workspaces.count())
            .map(|index| {
                let active = index == self.workspaces.active;
                let windows = if active {
                    self.space
                        .elements()
                        .filter(|w| self.window_space(w) == Some(index))
                        .filter_map(Self::window_id)
                        .collect()
                } else {
                    self.workspaces
                        .stored
                        .get(index)
                        .map(|placed| {
                            placed
                                .iter()
                                .filter_map(|(w, _)| Self::window_id(w))
                                .collect()
                        })
                        .unwrap_or_default()
                };
                SpaceInfo {
                    index,
                    active,
                    windows,
                }
            })
            .collect()
    }

    /// Una finestra di uno spazio non attivo.
    fn stored_window(&self, id: u64) -> Option<Window> {
        self.workspaces
            .stored
            .iter()
            .flatten()
            .find(|(w, _)| Self::window_id(w) == Some(id))
            .map(|(w, _)| w.clone())
    }

    /// Lo spazio non attivo che contiene la finestra `id`.
    fn stored_window_space(&self, id: u64) -> Option<usize> {
        self.workspaces
            .stored
            .iter()
            .position(|placed| placed.iter().any(|(w, _)| Self::window_id(w) == Some(id)))
    }

    pub(crate) fn focused_window(&self) -> Option<Window> {
        let focus = self.seat.get_keyboard()?.current_focus()?;
        if let Some(window) = self
            .space
            .elements()
            .find(|w| w.toplevel().is_some_and(|t| t.wl_surface() == &focus))
        {
            return Some(window.clone());
        }
        // Con la tastiera a un componente della shell (un menu della barra aperto) l'app attiva
        // resta quella in cima: i comandi dei menu agiscono su di lei.
        let on_layer = smithay::desktop::layer_map_for_output(&self.output)
            .layers()
            .any(|layer| layer.wl_surface() == &focus);
        if on_layer {
            return self.space.elements().last().cloned();
        }
        None
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
        let mut windows: Vec<(&Window, bool)> =
            self.space.elements().rev().map(|w| (w, false)).collect();
        windows.extend(self.minimized.iter().map(|(w, _)| (w, true)));
        // Poi le finestre degli altri spazi, ciascuna con la sua posizione nello spazio.
        let stored: Vec<(&Window, Point<i32, Logical>)> = self
            .workspaces
            .stored
            .iter()
            .flat_map(|placed| placed.iter().rev().map(|(w, l)| (w, *l)))
            .collect();
        windows.extend(stored.iter().map(|(w, _)| (*w, false)));
        windows
            .into_iter()
            .filter_map(|(window, minimized)| {
                let id = Self::window_id(window)?;
                let location = if minimized {
                    self.minimized
                        .iter()
                        .find(|(w, _)| w == window)
                        .map(|(_, location)| *location)
                        .unwrap_or_default()
                } else if let Some(location) = self.space.element_location(window) {
                    location
                } else {
                    stored
                        .iter()
                        .find(|(w, _)| *w == window)
                        .map(|(_, location)| *location)
                        .unwrap_or_default()
                };
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
                    minimized,
                    fullscreen: crate::windows::is_fullscreen(window),
                    space: self.window_space(window).unwrap_or(self.workspaces.active),
                })
            })
            .collect()
    }

    /// Termina (SIGKILL) i processi delle finestre di un'app: tutte quelle con quell'app_id, sullo
    /// schermo, ridotte o in altri spazi. Mai un componente della shell né Ossido stesso.
    fn force_quit(&mut self, app_id: &str) -> Result<usize, String> {
        let pids: std::collections::BTreeSet<i32> = self
            .space
            .elements()
            .chain(self.minimized.iter().map(|(w, _)| w))
            .chain(self.workspaces.stored.iter().flatten().map(|(w, _)| w))
            .filter(|w| crate::panorama::window_app(w).as_deref() == Some(app_id))
            .filter_map(|w| {
                let surface = w.toplevel()?.wl_surface().clone();
                let client = self.display_handle.get_client(surface.id()).ok()?;
                let data = client.get_data::<crate::state::ClientState>()?;
                (!data.shell).then_some(data.pid?)
            })
            .filter(|pid| *pid != std::process::id() as i32)
            .collect();
        if pids.is_empty() {
            return Err(format!("nessun processo per {app_id}"));
        }
        for pid in &pids {
            let Some(pid) = rustix::process::Pid::from_raw(*pid) else {
                continue;
            };
            if let Err(err) = rustix::process::kill_process(pid, rustix::process::Signal::KILL) {
                warn!(
                    "uscita forzata di {app_id} ({}): {err}",
                    pid.as_raw_nonzero()
                );
            }
        }
        Ok(pids.len())
    }

    /// Scrive l'istantanea in un file temporaneo e la annuncia alla shell (`broadcast`
    /// "screenshot" = "<percorso> <larghezza> <altezza>"): la miniatura, poi la Scrivania.
    fn publish_screenshot(&mut self, capture: &crate::capture::Capture) -> Result<Value, String> {
        let png = capture.to_png()?;
        let dir = std::env::var_os("XDG_RUNTIME_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        self.screenshot_count = self.screenshot_count.wrapping_add(1);
        let path = dir.join(format!(
            "silica-istantanea-{}-{}.png",
            std::process::id(),
            self.screenshot_count
        ));
        std::fs::write(&path, &png).map_err(|e| format!("scrittura di {}: {e}", path.display()))?;
        let path = path.to_string_lossy().into_owned();
        self.broadcast_to_shell(
            "screenshot".to_owned(),
            format!("{path} {} {}", capture.width, capture.height),
        );
        Ok(json!({ "path": path, "width": capture.width, "height": capture.height }))
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
