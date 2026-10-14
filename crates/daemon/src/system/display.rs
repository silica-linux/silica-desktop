//! Displays and the night light, for apps: `System1.Outputs`, `ConfigureOutput` and
//! `SetNightLight`.
//!
//! The displays belong to Ossido: the daemon passes the calls to its actions socket
//! (`output.list`, `output.configure`, `output.color_temperature`) and Ossido reports the list
//! back on every change (`displays` in the state, through `ReportCompositor`).
//!
//! The night light's settings are kept in `$XDG_CONFIG_HOME/silica/nightlight.json`; its
//! schedule runs here (`sun.rs`), and Ossido only receives the colour temperature to show.

use std::path::{Path, PathBuf};

use serde_json::{Value as Json, json};

use super::Shared;
use crate::sun;

/// The colour temperature with the night light off.
pub const NEUTRAL_KELVIN: u32 = 6500;
pub const MIN_KELVIN: u32 = 1900;
pub const DEFAULT_KELVIN: u32 = 3400;

/// `nightlight.json`.
#[derive(Clone, Debug, PartialEq)]
pub struct NightLight {
    pub enabled: bool,
    /// Kelvin, 1900–6500.
    pub temperature: u32,
    /// `off` (on while enabled), `sunset` (sunset to sunrise) or `custom` (`from`–`to`).
    pub schedule: String,
    pub from: String,
    pub to: String,
}

impl Default for NightLight {
    fn default() -> Self {
        Self {
            enabled: false,
            temperature: DEFAULT_KELVIN,
            schedule: "sunset".to_owned(),
            from: "22:00".to_owned(),
            to: "07:00".to_owned(),
        }
    }
}

impl NightLight {
    /// From the file's object; invalid keys keep their default.
    pub fn from_json(json: &Json) -> Self {
        let mut light = NightLight::default();
        light.merge(json);
        light
    }

    /// Sets the valid keys of `json`; returns false if one was invalid.
    pub fn merge(&mut self, json: &Json) -> bool {
        let mut valid = true;
        if let Some(value) = json.get("enabled") {
            match value.as_bool() {
                Some(b) => self.enabled = b,
                None => valid = false,
            }
        }
        if let Some(value) = json.get("temperature") {
            match value
                .as_u64()
                .and_then(|k| u32::try_from(k).ok())
                .filter(|k| (MIN_KELVIN..=NEUTRAL_KELVIN).contains(k))
            {
                Some(k) => self.temperature = k,
                None => valid = false,
            }
        }
        if let Some(value) = json.get("schedule") {
            match value.as_str() {
                Some(s @ ("off" | "sunset" | "custom")) => self.schedule = s.to_owned(),
                _ => valid = false,
            }
        }
        for (key, slot) in [("from", &mut self.from), ("to", &mut self.to)] {
            if let Some(value) = json.get(key) {
                match value.as_str().filter(|s| sun::parse_clock(s).is_some()) {
                    Some(s) => *slot = s.to_owned(),
                    None => valid = false,
                }
            }
        }
        valid
    }

    pub fn to_json(&self) -> Json {
        json!({
            "enabled": self.enabled,
            "temperature": self.temperature,
            "schedule": self.schedule,
            "from": self.from,
            "to": self.to,
        })
    }

    /// Whether the warm colours are on at `now` (Unix seconds, local offset `offset`).
    pub fn active(&self, place: Option<sun::Place>, now: i64, offset: i64) -> bool {
        if !self.enabled {
            return false;
        }
        match self.schedule.as_str() {
            "sunset" => sun::is_night(place, now, offset),
            "custom" => {
                let minute = (now + offset).rem_euclid(86_400) / 60;
                match (sun::parse_clock(&self.from), sun::parse_clock(&self.to)) {
                    (Some(from), Some(to)) => sun::in_window(minute, from, to),
                    _ => false,
                }
            }
            _ => true,
        }
    }

    /// The temperature Ossido shows.
    pub fn kelvin(&self, active: bool) -> u32 {
        if active {
            self.temperature
        } else {
            NEUTRAL_KELVIN
        }
    }
}

/// `$XDG_CONFIG_HOME/silica/<name>`.
pub fn config_file(name: &str) -> Option<PathBuf> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(config.join("silica").join(name))
}

pub fn read_night_light(file: &Path) -> NightLight {
    std::fs::read_to_string(file)
        .ok()
        .and_then(|text| serde_json::from_str::<Json>(&text).ok())
        .map(|json| NightLight::from_json(&json))
        .unwrap_or_default()
}

/// Writes a JSON file through a temporary one (a reader never sees half of it).
pub fn write_json(file: &Path, json: &Json) -> Result<(), String> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let temporary = file.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(json).map_err(|e| e.to_string())?;
    std::fs::write(&temporary, text)
        .and_then(|()| std::fs::rename(&temporary, file))
        .map_err(|e| format!("{}: {e}", file.display()))
}

/// The actions socket of Ossido: the configured one, or the one of `WAYLAND_DISPLAY`.
fn socket(shared: &Shared) -> Result<PathBuf, String> {
    if let Some(path) = &shared.config.actions_socket {
        return Ok(path.clone());
    }
    let display = std::env::var("WAYLAND_DISPLAY").map_err(|_| "no Wayland display".to_owned())?;
    silica_control::actions::socket_path(&display).ok_or_else(|| "no runtime folder".to_owned())
}

/// Runs an Ossido command through its actions socket, off the async executor.
pub(super) async fn ossido(
    shared: &Shared,
    command: &'static str,
    params: Json,
) -> Result<Json, String> {
    let path = socket(shared)?;
    blocking::unblock(move || {
        silica_control::actions::run_at(&path, command, params).map_err(|e| e.to_string())
    })
    .await
}

/// The night light now: read its file, compute the schedule, update the state and, when the
/// temperature changes (or `force`), tell Ossido.
pub(super) async fn night_light_tick(shared: &Shared, last: &mut Option<u32>, force: bool) {
    let light = shared
        .config
        .nightlight_file
        .as_deref()
        .map(read_night_light)
        .unwrap_or_else(|| shared.state().night_light.clone());
    let active = light.enabled && {
        // The local offset only matters to a schedule.
        let offset = match (light.schedule.as_str(), shared.config.utc_offset) {
            ("off", _) => 0,
            (_, Some(offset)) => offset,
            (_, None) => sun::local_offset().await,
        };
        light.active(shared.config.place, sun::unix_now(), offset)
    };
    let kelvin = light.kelvin(active);
    shared.update(|state| {
        state.night_light = light;
        state.night_light_active = active;
    });
    if force || *last != Some(kelvin) {
        match ossido(
            shared,
            silica_control::commands::OUTPUT_COLOR_TEMPERATURE,
            json!({ "kelvin": kelvin }),
        )
        .await
        {
            Ok(_) => *last = Some(kelvin),
            Err(err) => tracing::debug!("night light: {err}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn night_light_settings() {
        let light = NightLight::from_json(&json!({"enabled": true, "temperature": 2700,
            "schedule": "custom", "from": "21:30", "to": "06:15"}));
        assert!(light.enabled);
        assert_eq!(light.temperature, 2700);
        // 22:00 local (UTC+2) is inside 21:30–06:15, 12:00 is not.
        let day = 20_000 * 86_400;
        assert!(light.active(None, day + 20 * 3600, 7200));
        assert!(!light.active(None, day + 10 * 3600, 7200));
        assert_eq!(light.kelvin(true), 2700);
        assert_eq!(light.kelvin(false), NEUTRAL_KELVIN);

        let mut light = NightLight::default();
        assert!(!light.active(None, day, 0));
        assert!(!light.merge(&json!({"temperature": 100, "schedule": "never", "from": "25:00"})));
        assert_eq!(light, NightLight::default());
        assert!(light.merge(&json!({"enabled": true, "schedule": "off"})));
        assert!(light.active(None, day + 12 * 3600, 0));
        assert_eq!(NightLight::from_json(&light.to_json()), light);
    }
}
