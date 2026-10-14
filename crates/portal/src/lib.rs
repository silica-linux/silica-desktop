//! Il backend xdg-desktop-portal di Silica (`org.freedesktop.impl.portal.FileChooser`): i
//! dialoghi Apri e Salva delle app sono finestre di Mustache in modalità dialogo
//! (`--dialog=open|save`), avviate dalla sua voce .desktop; la scelta torna sullo stdout, un
//! URI per riga (vuoto e uscita non zero: annullato). Specifica: documentazione di
//! xdg-desktop-portal, interfaccia `org.freedesktop.impl.portal.FileChooser`.

pub mod settings;

use std::collections::HashMap;
use std::path::Path;

/// Il nome del backend sul bus di sessione e l'oggetto dei portali.
pub const BUS_NAME: &str = "org.freedesktop.impl.portal.desktop.silica";
pub const OBJECT_PATH: &str = "/org/freedesktop/portal/desktop";

/// La voce .desktop di Mustache.
pub const MUSTACHE_ID: &str = "dev.frascella.Mustache.desktop";

/// Le risposte del portale: 0 fatto, 1 annullato dall'utente, 2 altro errore.
pub const RESPONSE_OK: u32 = 0;
pub const RESPONSE_CANCELLED: u32 = 1;
pub const RESPONSE_OTHER: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogKind {
    Open,
    Save,
}

/// Quel che chiede l'app.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DialogRequest {
    pub title: String,
    pub accept_label: Option<String>,
    pub multiple: bool,
    pub directory: bool,
    pub current_name: Option<String>,
    pub current_folder: Option<String>,
}

/// Le parole di una riga `Exec` (virgolette della specifica Desktop Entry), senza i codici di
/// campo: i file del dialogo non c'entrano.
pub fn exec_words(exec: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut started = false;
    let mut chars = exec.chars();
    while let Some(c) = chars.next() {
        if quoted {
            match c {
                '\\' => {
                    if let Some(next) = chars.next() {
                        current.push(next);
                    }
                }
                '"' => quoted = false,
                _ => current.push(c),
            }
        } else if c == '"' {
            quoted = true;
            started = true;
        } else if c == ' ' || c == '\t' {
            if started || !current.is_empty() {
                out.push(std::mem::take(&mut current));
            }
            started = false;
        } else {
            current.push(c);
            started = true;
        }
    }
    if started || !current.is_empty() {
        out.push(current);
    }
    out.into_iter()
        .filter(|w| !(w.len() == 2 && w.starts_with('%')))
        .collect()
}

/// Il valore di `Exec` nella sezione `[Desktop Entry]`.
pub fn desktop_exec(text: &str) -> Option<String> {
    let mut in_entry = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if in_entry && let Some(value) = line.strip_prefix("Exec=") {
            return Some(value.trim().to_owned());
        }
    }
    None
}

/// La riga di comando di Mustache per il dialogo: `Exec` più gli argomenti per Dart.
pub fn dialog_command(exec: &str, kind: DialogKind, request: &DialogRequest) -> Vec<String> {
    let mut words = exec_words(exec);
    let mut arg = |value: String| {
        words.push("--dart-arg".to_owned());
        words.push(value);
    };
    arg(match kind {
        DialogKind::Open => "--dialog=open".to_owned(),
        DialogKind::Save => "--dialog=save".to_owned(),
    });
    if !request.title.is_empty() {
        arg(format!("--title={}", request.title));
    }
    if let Some(label) = &request.accept_label {
        arg(format!("--accept={label}"));
    }
    if request.multiple {
        arg("--multiple".to_owned());
    }
    if request.directory {
        arg("--directory".to_owned());
    }
    if let Some(name) = &request.current_name {
        arg(format!("--name={name}"));
    }
    if let Some(folder) = &request.current_folder {
        arg(format!("--folder={folder}"));
    }
    words
}

/// Gli URI scelti dallo stdout di Mustache: solo righe `file://`.
pub fn parse_uris(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("file://"))
        .map(str::to_owned)
        .collect()
}

/// Un percorso dato come array di byte terminato da zero (`current_folder` del portale).
pub fn path_from_bytes(bytes: &[u8]) -> Option<String> {
    let trimmed = bytes.strip_suffix(&[0]).unwrap_or(bytes);
    let text = std::str::from_utf8(trimmed).ok()?;
    (!text.is_empty() && Path::new(text).is_absolute()).then(|| text.to_owned())
}

/// La voce di Mustache nelle cartelle dei dati XDG (la prima vince).
pub fn find_mustache_exec(env: &HashMap<String, String>) -> Option<String> {
    let home = env.get("HOME").cloned().unwrap_or_default();
    let data_home = env
        .get("XDG_DATA_HOME")
        .filter(|v| !v.is_empty())
        .cloned()
        .unwrap_or_else(|| format!("{home}/.local/share"));
    let data_dirs = env
        .get("XDG_DATA_DIRS")
        .filter(|v| !v.is_empty())
        .cloned()
        .unwrap_or_else(|| "/usr/local/share:/usr/share".to_owned());
    std::iter::once(data_home)
        .chain(data_dirs.split(':').map(str::to_owned))
        .filter(|d| !d.is_empty())
        .find_map(|dir| {
            let text = std::fs::read_to_string(format!("{dir}/applications/{MUSTACHE_ID}")).ok()?;
            desktop_exec(&text)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exec_words_respect_quotes_and_drop_field_codes() {
        assert_eq!(
            exec_words(r#"/opt/silica/embedder --title "Il Mustache" /b/flutter_assets %F"#),
            [
                "/opt/silica/embedder",
                "--title",
                "Il Mustache",
                "/b/flutter_assets"
            ]
        );
        assert_eq!(exec_words(r#"a "b \"c\"""#), ["a", r#"b "c""#]);
    }

    #[test]
    fn the_dialog_command_adds_dart_arguments() {
        let request = DialogRequest {
            title: "Scegli".to_owned(),
            multiple: true,
            current_folder: Some("/home/io".to_owned()),
            ..DialogRequest::default()
        };
        let words = dialog_command("emb --app-id x /b", DialogKind::Open, &request);
        assert_eq!(
            words,
            [
                "emb",
                "--app-id",
                "x",
                "/b",
                "--dart-arg",
                "--dialog=open",
                "--dart-arg",
                "--title=Scegli",
                "--dart-arg",
                "--multiple",
                "--dart-arg",
                "--folder=/home/io",
            ]
        );
    }

    #[test]
    fn results_and_paths() {
        assert_eq!(
            parse_uris("file:///a.txt\nrumore\n file:///b%20c.txt \n"),
            ["file:///a.txt", "file:///b%20c.txt"]
        );
        assert_eq!(path_from_bytes(b"/home/io\0").as_deref(), Some("/home/io"));
        assert_eq!(path_from_bytes(b"relativo\0"), None);
        assert_eq!(path_from_bytes(&[0xff, 0]), None);
        assert_eq!(
            desktop_exec("[Desktop Action x]\nExec=no\n[Desktop Entry]\nName=M\nExec=emb /b\n")
                .as_deref(),
            Some("emb /b")
        );
    }

    #[test]
    fn the_desktop_entry_is_found_in_the_data_dirs() {
        let dir = std::env::temp_dir().join(format!("silica-portal-{}", std::process::id()));
        let apps = dir.join("applications");
        std::fs::create_dir_all(&apps).unwrap();
        std::fs::write(
            apps.join(MUSTACHE_ID),
            "[Desktop Entry]\nType=Application\nExec=emb /b\n",
        )
        .unwrap();
        let env = HashMap::from([
            ("HOME".to_owned(), "/nessuna".to_owned()),
            ("XDG_DATA_DIRS".to_owned(), dir.display().to_string()),
        ]);
        assert_eq!(find_mustache_exec(&env).as_deref(), Some("emb /b"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
