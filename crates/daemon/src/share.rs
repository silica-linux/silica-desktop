//! The share service: `dev.frascella.Silica.Share1` on `/dev/frascella/Silica/Share1`.
//!
//! An app's Share button asks for the targets of a list of files, shows them in its Share menu
//! and sends the chosen one back. The targets are:
//! - `copy`: the files on the clipboard (`text/uri-list`, through the clipboard service);
//! - every application whose desktop entry opts in with `X-Silica-Share=true` (Mail, for
//!   example). It is started with the files, following its `Exec` field codes.
//!
//! The menu itself (its look and order) belongs to the app's toolkit (silica_ui).

use std::path::{Path, PathBuf};

use serde::Serialize;
use zbus::interface;

use crate::clipboard;

pub const SHARE_PATH: &str = "/dev/frascella/Silica/Share1";

/// The desktop entry key an application sets to appear among the share targets.
pub const SHARE_KEY: &str = "X-Silica-Share";

/// A target of the Share menu.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Target {
    /// `copy`, or the desktop entry id of an application (`dev.frascella.Mail`).
    pub id: String,
    /// The name to show (an application's `Name`; the toolkit translates `copy`).
    pub name: String,
    /// The application's `Icon` (an id or a path), empty for `copy`.
    pub icon: String,
}

/// An application that takes shared files.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShareApp {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub exec: String,
}

/// The `applications/` folders, most important first (XDG_DATA_HOME, then XDG_DATA_DIRS).
fn application_dirs(env: &dyn Fn(&str) -> Option<String>) -> Vec<PathBuf> {
    let home = env("HOME").unwrap_or_default();
    let data_home = env("XDG_DATA_HOME")
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| format!("{home}/.local/share"));
    let data_dirs = env("XDG_DATA_DIRS")
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".to_owned());
    std::iter::once(data_home)
        .chain(
            data_dirs
                .split(':')
                .filter(|d| !d.is_empty())
                .map(str::to_owned),
        )
        .map(|d| Path::new(&d).join("applications"))
        .collect()
}

/// The share app of the `[Desktop Entry]` group of `text`, or None when the entry is not an
/// application, is hidden, has no Exec, or does not opt in.
pub fn parse_entry(id: &str, text: &str) -> Option<ShareApp> {
    let mut in_entry = false;
    let (mut name, mut icon, mut exec, mut kind) = (None, String::new(), None, None);
    let (mut hidden, mut share) = (false, false);
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        match key {
            "Name" => name = Some(value.to_owned()),
            "Icon" => icon = value.to_owned(),
            "Exec" => exec = Some(value.to_owned()),
            "Type" => kind = Some(value.to_owned()),
            "Hidden" | "NoDisplay" if value == "true" => hidden = true,
            SHARE_KEY => share = value == "true",
            _ => {}
        }
    }
    if hidden || !share || kind.as_deref() != Some("Application") {
        return None;
    }
    Some(ShareApp {
        id: id.to_owned(),
        name: name.unwrap_or_else(|| id.to_owned()),
        icon,
        exec: exec.filter(|e| !e.is_empty())?,
    })
}

/// The applications that opt in, by name; an id found in an earlier folder wins.
pub fn share_apps(env: &dyn Fn(&str) -> Option<String>) -> Vec<ShareApp> {
    let mut apps: Vec<ShareApp> = Vec::new();
    for dir in application_dirs(env) {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
        paths.sort();
        for path in paths {
            let Some(id) = path
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.strip_suffix(".desktop"))
            else {
                continue;
            };
            if apps.iter().any(|a| a.id == id) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            if let Some(app) = parse_entry(id, &text) {
                apps.push(app);
            }
        }
    }
    apps.sort_by_key(|a| a.name.to_lowercase());
    apps
}

/// Splits an `Exec` line into words (double quotes group, `\` escapes inside them).
fn split_exec(exec: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let (mut quoted, mut started) = (false, false);
    let mut chars = exec.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                quoted = !quoted;
                started = true;
            }
            '\\' if quoted => {
                if let Some(next) = chars.next() {
                    word.push(next);
                }
            }
            c if c.is_whitespace() && !quoted => {
                if started || !word.is_empty() {
                    words.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            c => word.push(c),
        }
    }
    if started || !word.is_empty() {
        words.push(word);
    }
    words
}

/// The local path of a `file://` URI (percent-decoded), or the argument itself.
fn local_path(uri: &str) -> String {
    let Some(rest) = uri.strip_prefix("file://") else {
        return uri.to_owned();
    };
    let path = rest.strip_prefix("localhost").unwrap_or(rest);
    let mut out = Vec::with_capacity(path.len());
    let mut bytes = path.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'%' {
            let mut ahead = bytes.clone();
            let hex = [ahead.next(), ahead.next()];
            if let [Some(hi), Some(lo)] = hex
                && let (Some(hi), Some(lo)) = ((hi as char).to_digit(16), (lo as char).to_digit(16))
            {
                out.push((hi * 16 + lo) as u8);
                bytes = ahead;
                continue;
            }
        }
        out.push(byte);
    }
    String::from_utf8(out).unwrap_or_else(|_| path.to_owned())
}

/// The command lines that hand `uris` to `exec`, following the field codes: `%F`/`%U` take
/// every file in one run, `%f`/`%u` one run per file, and an Exec without them gets the files
/// at the end. `%%` is a `%`; the other codes are dropped.
pub fn command_lines(exec: &str, uris: &[String]) -> Vec<Vec<String>> {
    let words = split_exec(exec);
    let paths: Vec<String> = uris.iter().map(|u| local_path(u)).collect();
    let has = |code: &str| words.iter().any(|w| w == code);
    let expand = |single: Option<&String>| -> Vec<String> {
        let mut out = Vec::new();
        for word in &words {
            match word.as_str() {
                "%F" => out.extend(paths.iter().cloned()),
                "%U" => out.extend(uris.iter().cloned()),
                "%f" => out.extend(single.map(|u| local_path(u))),
                "%u" => out.extend(single.cloned()),
                w if w.len() == 2 && w.starts_with('%') && w != "%%" => {}
                w => out.push(w.replace("%%", "%")),
            }
        }
        out
    };
    if has("%f") || has("%u") {
        return uris.iter().map(|u| expand(Some(u))).collect();
    }
    let mut line = expand(None);
    if !has("%F") && !has("%U") {
        line.extend(paths);
    }
    vec![line]
}

pub struct Share;

#[interface(name = "dev.frascella.Silica.Share1")]
impl Share {
    /// The targets for `uris`, as JSON: `[{id, name, icon}]`, `copy` first. Empty when
    /// there is nothing to share.
    fn targets(&self, uris: Vec<String>) -> String {
        if uris.is_empty() {
            return "[]".to_owned();
        }
        let env = |k: &str| std::env::var(k).ok();
        let mut targets = vec![Target {
            id: "copy".to_owned(),
            name: "Copy".to_owned(),
            icon: String::new(),
        }];
        targets.extend(share_apps(&env).into_iter().map(|a| Target {
            id: a.id,
            name: a.name,
            icon: a.icon,
        }));
        serde_json::to_string(&targets).unwrap_or_else(|_| "[]".to_owned())
    }

    /// Shares `uris` with `target` (an id from `Targets`). False if the target is unknown or
    /// could not start.
    fn share(&self, target: String, uris: Vec<String>) -> bool {
        if uris.is_empty() {
            return false;
        }
        if target == "copy" {
            return clipboard::copy_files(&uris);
        }
        let env = |k: &str| std::env::var(k).ok();
        let Some(app) = share_apps(&env).into_iter().find(|a| a.id == target) else {
            return false;
        };
        let mut started = false;
        for line in command_lines(&app.exec, &uris) {
            let Some((program, args)) = line.split_first() else {
                continue;
            };
            match std::process::Command::new(program).args(args).spawn() {
                Ok(child) => {
                    started = true;
                    // Reap it in the background so it does not stay a zombie.
                    let _ = std::thread::Builder::new()
                        .name("share-child".to_owned())
                        .spawn(move || {
                            let mut child = child;
                            let _ = child.wait();
                        });
                }
                Err(err) => eprintln!("silica-daemon: share with {}: {err}", app.id),
            }
        }
        started
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const MAIL: &str = "[Desktop Entry]\nType=Application\nName=Mail\nIcon=dev.frascella.Mail\nExec=mail --compose %F\nX-Silica-Share=true\n";

    #[test]
    fn only_apps_that_opt_in_are_targets() {
        let app = parse_entry("dev.frascella.Mail", MAIL);
        assert_eq!(
            app,
            Some(ShareApp {
                id: "dev.frascella.Mail".to_owned(),
                name: "Mail".to_owned(),
                icon: "dev.frascella.Mail".to_owned(),
                exec: "mail --compose %F".to_owned(),
            })
        );
        assert_eq!(
            parse_entry("x", &MAIL.replace("X-Silica-Share=true\n", "")),
            None
        );
        assert_eq!(parse_entry("x", &format!("{MAIL}NoDisplay=true\n")), None);
        assert_eq!(
            parse_entry("x", &MAIL.replace("Type=Application", "Type=Link")),
            None
        );
        // Keys of other groups don't count.
        let action = "[Desktop Entry]\nType=Application\nName=A\nExec=a\n[Desktop Action b]\nX-Silica-Share=true\n";
        assert_eq!(parse_entry("a", action), None);
    }

    #[test]
    fn field_codes_decide_how_files_are_passed() {
        let uris = vec![
            "file:///home/a/My%20File.png".to_owned(),
            "file:///home/a/b.txt".to_owned(),
        ];
        assert_eq!(
            command_lines("mail --compose %F", &uris),
            vec![vec![
                "mail",
                "--compose",
                "/home/a/My File.png",
                "/home/a/b.txt"
            ]]
        );
        assert_eq!(
            command_lines("viewer %u", &uris),
            vec![
                vec!["viewer", "file:///home/a/My%20File.png"],
                vec!["viewer", "file:///home/a/b.txt"],
            ]
        );
        assert_eq!(
            command_lines("\"/opt/my app/run\" --x", &uris[1..]),
            vec![vec!["/opt/my app/run", "--x", "/home/a/b.txt"]]
        );
        assert_eq!(
            command_lines("tool %i %c 100%% %F", &uris[1..]),
            vec![vec!["tool", "100%", "/home/a/b.txt"]]
        );
    }

    #[test]
    fn earlier_folders_win_and_apps_are_sorted() {
        let root = std::env::temp_dir().join(format!("share-apps-{}", std::process::id()));
        let (home, sys) = (root.join("home"), root.join("sys"));
        for dir in [&home, &sys] {
            std::fs::create_dir_all(dir.join("applications")).unwrap();
        }
        std::fs::write(home.join("applications/dev.frascella.Mail.desktop"), MAIL).unwrap();
        std::fs::write(
            sys.join("applications/dev.frascella.Mail.desktop"),
            MAIL.replace("Name=Mail", "Name=Old"),
        )
        .unwrap();
        std::fs::write(
            sys.join("applications/a.desktop"),
            MAIL.replace("Name=Mail", "Name=Archive"),
        )
        .unwrap();
        let (h, s) = (home.display().to_string(), sys.display().to_string());
        let env = move |k: &str| match k {
            "XDG_DATA_HOME" => Some(h.clone()),
            "XDG_DATA_DIRS" => Some(s.clone()),
            _ => None,
        };
        let names: Vec<String> = share_apps(&env).into_iter().map(|a| a.name).collect();
        assert_eq!(names, vec!["Archive", "Mail"]);
        let _ = std::fs::remove_dir_all(&root);
    }
}
