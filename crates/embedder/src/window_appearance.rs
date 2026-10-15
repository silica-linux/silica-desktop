//! The window's appearance override from Dart (`silica/window` › `setAppearance`) to
//! `silica-window-v1` › `set_appearance`, and the appearance Ossido reports for the
//! arrangement popover (`silica/shell` › `windowMenuAppearance`).

use serde_json::Value;
use silica_protocols::window::client::silica_window_v1::Appearance;

/// The `appearance` argument of `setAppearance`: `"system"`, `"light"` or `"dark"`.
/// Anything else is not understood (the call answers `false`).
pub fn parse(args: Option<&Value>) -> Option<Appearance> {
    match args?.get("appearance")?.as_str()? {
        "system" => Some(Appearance::System),
        "light" => Some(Appearance::Light),
        "dark" => Some(Appearance::Dark),
        _ => None,
    }
}

/// The name Dart uses for the value Ossido reports with the popover (0 system, 1 light,
/// 2 dark); an unknown value is the system's.
pub fn name(value: u32) -> &'static str {
    match value {
        1 => "light",
        2 => "dark",
        _ => "system",
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use serde_json::json;

    #[test]
    fn the_three_appearances_are_understood() {
        let of = |name: &str| parse(Some(&json!({ "appearance": name })));
        assert_eq!(of("system"), Some(Appearance::System));
        assert_eq!(of("light"), Some(Appearance::Light));
        assert_eq!(of("dark"), Some(Appearance::Dark));
    }

    #[test]
    fn anything_else_is_refused() {
        assert_eq!(parse(None), None);
        assert_eq!(parse(Some(&json!(null))), None);
        assert_eq!(parse(Some(&json!({}))), None);
        assert_eq!(parse(Some(&json!({ "appearance": "Dark" }))), None);
        assert_eq!(parse(Some(&json!({ "appearance": 2 }))), None);
    }

    #[test]
    fn the_popover_appearance_has_a_name() {
        assert_eq!(name(0), "system");
        assert_eq!(name(1), "light");
        assert_eq!(name(2), "dark");
        assert_eq!(name(9), "system");
    }
}
