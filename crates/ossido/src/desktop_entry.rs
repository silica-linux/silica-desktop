//! Desktop entries for `app.launch {app}`: the id of an application, its `Exec` with the field
//! codes stripped, as the Dock and Faro run it (so `--app-id` and the like are kept).

use std::path::{Path, PathBuf};

/// The `applications/` folders to look in: XDG_DATA_HOME, XDG_DATA_DIRS, then the Flatpak
/// exports (user and system) when the session does not list them.
fn application_dirs(env: &dyn Fn(&str) -> Option<String>) -> Vec<PathBuf> {
    let home = env("HOME").unwrap_or_default();
    let data_home = env("XDG_DATA_HOME")
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| format!("{home}/.local/share"));
    let data_dirs = env("XDG_DATA_DIRS")
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".to_owned());
    let mut dirs: Vec<String> = vec![data_home];
    dirs.extend(
        data_dirs
            .split(':')
            .filter(|d| !d.is_empty())
            .map(str::to_owned),
    );
    for flatpak in [
        format!("{home}/.local/share/flatpak/exports/share"),
        "/var/lib/flatpak/exports/share".to_owned(),
    ] {
        if !flatpak.starts_with('/') {
            continue;
        }
        if !dirs.iter().any(|d| d.trim_end_matches('/') == flatpak) {
            dirs.push(flatpak);
        }
    }
    dirs.into_iter()
        .map(|d| Path::new(&d).join("applications"))
        .collect()
}

/// Removes the Exec field codes (`%f %F %u %U %d %D %n %N %i %c %k %v %m`); `%%` is a `%`.
pub fn strip_field_codes(exec: &str) -> String {
    let mut out = String::with_capacity(exec.len());
    let mut chars = exec.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' && chars.peek().is_some() {
            if chars.next() == Some('%') {
                out.push('%');
            }
            continue;
        }
        out.push(c);
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The `Exec` of the `[Desktop Entry]` group of `text`, with the field codes stripped. None for
/// an entry that is not an application, is hidden, or has no Exec.
pub fn exec_of(text: &str) -> Option<String> {
    let mut in_entry = false;
    let (mut exec, mut kind, mut hidden) = (None, None, false);
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key.trim() {
            "Exec" if exec.is_none() => exec = Some(value.trim().to_owned()),
            "Type" if kind.is_none() => kind = Some(value.trim().to_owned()),
            "Hidden" if value.trim() == "true" => hidden = true,
            _ => {}
        }
    }
    if hidden || kind.as_deref() != Some("Application") {
        return None;
    }
    Some(strip_field_codes(&exec?)).filter(|e| !e.is_empty())
}

/// The command that runs the app `id` (`org.mozilla.firefox`, with or without `.desktop`),
/// from the first data folder that has its entry.
pub fn command_for(id: &str, env: &dyn Fn(&str) -> Option<String>) -> Result<String, String> {
    let id = id.strip_suffix(".desktop").unwrap_or(id);
    if id.is_empty()
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '+'))
    {
        return Err(format!("not a desktop entry id: {id}"));
    }
    for dir in application_dirs(env) {
        // The first folder wins, even if the entry is hidden there.
        let Ok(text) = std::fs::read_to_string(dir.join(format!("{id}.desktop"))) else {
            continue;
        };
        return exec_of(&text).ok_or_else(|| format!("{id}: not a launchable application"));
    }
    Err(format!("no desktop entry for {id}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_codes_are_dropped() {
        assert_eq!(
            strip_field_codes("silica-calculator --app-id x %U  %%done"),
            "silica-calculator --app-id x %done"
        );
        assert_eq!(
            strip_field_codes("flatpak run org.mozilla.firefox @@u %u @@"),
            "flatpak run org.mozilla.firefox @@u @@"
        );
    }

    #[test]
    fn exec_of_reads_the_main_group_only() {
        let text = "[Desktop Entry]\nType=Application\nName=Calc\nExec=calc --app-id dev.x %f\n\n[Desktop Action n]\nExec=other\n";
        assert_eq!(exec_of(text).as_deref(), Some("calc --app-id dev.x"));
        assert_eq!(
            exec_of("[Desktop Entry]\nType=Application\nHidden=true\nExec=a\n"),
            None
        );
        assert_eq!(exec_of("[Desktop Entry]\nType=Link\nExec=a\n"), None);
    }

    #[test]
    fn command_for_searches_the_data_dirs() {
        let root = std::env::temp_dir().join(format!("silica-entry-{}", std::process::id()));
        let apps = root.join("share/applications");
        std::fs::create_dir_all(&apps).unwrap();
        std::fs::write(
            apps.join("dev.x.Calc.desktop"),
            "[Desktop Entry]\nType=Application\nName=C\nExec=calc --app-id dev.x.Calc %U\n",
        )
        .unwrap();
        let share = root.join("share").to_string_lossy().into_owned();
        let env = move |k: &str| match k {
            "XDG_DATA_DIRS" => Some(share.clone()),
            "HOME" => Some("/nonexistent".to_owned()),
            _ => None,
        };
        assert_eq!(
            command_for("dev.x.Calc", &env).unwrap(),
            "calc --app-id dev.x.Calc"
        );
        assert_eq!(
            command_for("dev.x.Calc.desktop", &env).unwrap(),
            "calc --app-id dev.x.Calc"
        );
        assert!(command_for("missing", &env).is_err());
        assert!(command_for("../etc/passwd", &env).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
