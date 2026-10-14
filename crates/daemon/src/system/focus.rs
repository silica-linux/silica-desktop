//! Focus modes: Do Not Disturb and the user's own modes from `$XDG_CONFIG_HOME/silica/focus.json`
//! (written by Settings), one active at a time (`System1.SetFocus`).
//!
//! ```json
//! {"modes": [{"id": "work", "name": "Work", "symbol": "briefcase",
//!             "allowedApps": ["dev.frascella.Mail"], "allowedPeople": []}],
//!  "active": "work"}
//! ```

use std::path::Path;

use serde_json::{Value, json};

/// A Focus mode.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FocusMode {
    pub id: String,
    pub name: String,
    /// A silica-icons symbol name.
    pub symbol: String,
    /// Desktop ids of the apps whose notifications still show.
    pub allowed_apps: Vec<String>,
    pub allowed_people: Vec<String>,
}

impl FocusMode {
    /// The built-in mode.
    pub fn do_not_disturb() -> Self {
        Self {
            id: "dnd".to_owned(),
            name: "Do Not Disturb".to_owned(),
            symbol: "moon-fill".to_owned(),
            allowed_apps: Vec::new(),
            allowed_people: Vec::new(),
        }
    }

    fn from_json(json: &Value) -> Option<Self> {
        let text = |key: &str| {
            json.get(key)
                .and_then(Value::as_str)
                .filter(|s| s.len() <= 256)
                .map(str::to_owned)
        };
        let list = |key: &str| -> Vec<String> {
            json.get(key)
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .filter(|s| s.len() <= 256)
                        .take(256)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default()
        };
        let id = text("id").filter(|id| !id.is_empty() && id != "dnd")?;
        Some(Self {
            name: text("name").unwrap_or_else(|| id.clone()),
            symbol: text("symbol").unwrap_or_default(),
            allowed_apps: list("allowedApps"),
            allowed_people: list("allowedPeople"),
            id,
        })
    }
}

fn read(file: &Path) -> Value {
    std::fs::read_to_string(file)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}))
}

/// The user's modes in `focus.json` (at most 32; Do Not Disturb is always there besides).
pub fn read_modes(file: &Path) -> Vec<FocusMode> {
    read(file)
        .get("modes")
        .and_then(Value::as_array)
        .map(|modes| {
            modes
                .iter()
                .filter_map(FocusMode::from_json)
                .take(32)
                .collect()
        })
        .unwrap_or_default()
}

/// `dnd` or one of `modes`.
pub fn find(modes: &[FocusMode], id: &str) -> Option<FocusMode> {
    if id == "dnd" {
        return Some(FocusMode::do_not_disturb());
    }
    modes.iter().find(|m| m.id == id).cloned()
}

/// Saves the active mode in the file, keeping its modes.
pub fn save_active(file: &Path, id: &str) -> Result<(), String> {
    let mut json = read(file);
    if let Some(map) = json.as_object_mut() {
        map.insert("active".to_owned(), Value::from(id));
    }
    super::display::write_json(file, &json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_from_the_file() {
        let dir = std::env::temp_dir().join(format!("silica-focus-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("focus.json");
        std::fs::write(
            &file,
            r#"{"modes": [{"id": "work", "name": "Work", "allowedApps": ["a", 3]},
                          {"name": "no id"}, {"id": "dnd"}], "other": 1}"#,
        )
        .unwrap();
        let modes = read_modes(&file);
        assert_eq!(modes.len(), 1);
        assert_eq!(modes[0].allowed_apps, ["a"]);
        assert_eq!(find(&modes, "work").unwrap().name, "Work");
        assert_eq!(find(&modes, "dnd").unwrap().id, "dnd");
        assert!(find(&modes, "sleep").is_none());
        save_active(&file, "work").unwrap();
        let saved = read(&file);
        assert_eq!(saved["active"], "work");
        assert_eq!(saved["other"], 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
