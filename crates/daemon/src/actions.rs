//! The system actions Faro offers in its actions scope:
//! `dev.frascella.Silica.Actions1` on `/dev/frascella/Silica/Actions1`.
//!
//! They come from Ossido's command catalog (`silica-control`), so the list, the labels and
//! the shortcuts always match what the compositor runs. Only commands a person would run by
//! name, needing no parameters, are listed. Faro runs them with `engine.execute`, through the
//! shell channel.

use serde_json::{Value, json};
use zbus::interface;

pub const ACTIONS_PATH: &str = "/dev/frascella/Silica/Actions1";

/// The commands offered as actions, in the order Faro lists them when nothing is typed.
pub const ACTION_IDS: &[&str] = &[
    "screenshot.screen",
    "screenshot.selection",
    "screenshot.toolbar",
    "system.dark_toggle",
    "system.mute_toggle",
    "system.volume_up",
    "system.volume_down",
    "system.brightness_up",
    "system.brightness_down",
    "panorama.toggle",
    "panorama.app",
    "desktop.show",
    "space.add",
    "space.next",
    "space.previous",
    "launcher.toggle",
    "notifications.toggle",
    "controlcenter.toggle",
    "window.minimize",
    "window.fullscreen",
    "window.tile_left",
    "window.tile_right",
    "window.tile_top",
    "window.tile_bottom",
    "window.tile_fill",
    "window.arrange_2",
    "window.arrange_3",
    "window.arrange_4",
    "app.force_quit_panel",
    "session.quit",
];

/// The actions as JSON: `[{id, label, description, shortcut}]`, from the catalog.
pub fn actions_json() -> String {
    let catalog = silica_control::commands::catalog_json();
    let specs = catalog.as_array().cloned().unwrap_or_default();
    let list: Vec<Value> = ACTION_IDS
        .iter()
        .filter_map(|id| {
            specs
                .iter()
                .find(|spec| spec.get("id").and_then(Value::as_str) == Some(id))
        })
        .map(|spec| {
            json!({
                "id": spec.get("id"),
                "label": spec.get("label"),
                "description": spec.get("description"),
                "shortcut": spec.get("shortcut"),
            })
        })
        .collect();
    Value::Array(list).to_string()
}

pub struct Actions;

#[interface(name = "dev.frascella.Silica.Actions1")]
impl Actions {
    /// The actions, as JSON.
    fn list(&self) -> String {
        actions_json()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_action_is_in_the_catalog_and_needs_no_parameters() {
        let list: Vec<Value> = serde_json::from_str(&actions_json()).unwrap_or_default();
        assert_eq!(list.len(), ACTION_IDS.len());
        for id in ACTION_IDS {
            let spec = silica_control::commands::find(id);
            assert!(spec.is_some(), "{id} is not in the catalog");
            let params = spec.map(|s| s.to_json()).unwrap_or_default();
            let required = params
                .pointer("/params/required")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            assert_eq!(required, 0, "{id} needs parameters");
        }
    }
}
