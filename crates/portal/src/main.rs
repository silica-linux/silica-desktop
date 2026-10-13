//! `silica-portal`: il backend xdg-desktop-portal di Silica sul bus di sessione.

use std::collections::HashMap;
use std::process::{ExitCode, Stdio};
use std::time::Duration;

use silica_portal::{
    BUS_NAME, DialogKind, DialogRequest, OBJECT_PATH, RESPONSE_CANCELLED, RESPONSE_OK,
    RESPONSE_OTHER, dialog_command, find_mustache_exec, parse_uris, path_from_bytes,
};
use zbus::interface;
use zbus::zvariant::{ObjectPath, OwnedValue, Value};

type Results = HashMap<String, OwnedValue>;

struct FileChooser;

fn option_bool(options: &HashMap<String, OwnedValue>, key: &str) -> bool {
    options
        .get(key)
        .and_then(|v| bool::try_from(v).ok())
        .unwrap_or(false)
}

fn option_string(options: &HashMap<String, OwnedValue>, key: &str) -> Option<String> {
    options
        .get(key)
        .and_then(|v| String::try_from(v.try_clone().ok()?).ok())
        .filter(|s| !s.is_empty())
}

fn option_path(options: &HashMap<String, OwnedValue>, key: &str) -> Option<String> {
    let value = options.get(key)?.try_clone().ok()?;
    let bytes = Vec::<u8>::try_from(value).ok()?;
    path_from_bytes(&bytes)
}

fn request(title: &str, options: &HashMap<String, OwnedValue>) -> DialogRequest {
    DialogRequest {
        title: title.to_owned(),
        accept_label: option_string(options, "accept_label"),
        multiple: option_bool(options, "multiple"),
        directory: option_bool(options, "directory"),
        current_name: option_string(options, "current_name"),
        current_folder: option_path(options, "current_folder"),
    }
}

/// Apre il dialogo di Mustache e aspetta la scelta, senza bloccare il bus.
async fn run_dialog(kind: DialogKind, request: DialogRequest) -> (u32, Results) {
    let env: HashMap<String, String> = std::env::vars().collect();
    let Some(exec) = find_mustache_exec(&env) else {
        eprintln!(
            "silica-portal: Mustache non è installato ({})",
            silica_portal::MUSTACHE_ID
        );
        return (RESPONSE_OTHER, Results::new());
    };
    let words = dialog_command(&exec, kind, &request);
    let Some((program, args)) = words.split_first() else {
        return (RESPONSE_OTHER, Results::new());
    };
    let child = std::process::Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();
    let mut child = match child {
        Ok(child) => child,
        Err(err) => {
            eprintln!("silica-portal: {program}: {err}");
            return (RESPONSE_OTHER, Results::new());
        }
    };
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => async_io::Timer::after(Duration::from_millis(50)).await,
            Err(err) => {
                eprintln!("silica-portal: {err}");
                return (RESPONSE_OTHER, Results::new());
            }
        };
    };
    let mut stdout = String::new();
    if let Some(mut out) = child.stdout.take() {
        use std::io::Read;
        if out.read_to_string(&mut stdout).is_err() {
            stdout.clear();
        }
    }
    let uris = parse_uris(&stdout);
    if !status.success() || uris.is_empty() {
        return (RESPONSE_CANCELLED, Results::new());
    }
    let mut results = Results::new();
    if let Ok(value) = OwnedValue::try_from(Value::from(uris)) {
        results.insert("uris".to_owned(), value);
    }
    (RESPONSE_OK, results)
}

#[interface(name = "org.freedesktop.impl.portal.FileChooser")]
impl FileChooser {
    async fn open_file(
        &self,
        _handle: ObjectPath<'_>,
        _app_id: &str,
        _parent_window: &str,
        title: &str,
        options: HashMap<String, OwnedValue>,
    ) -> (u32, Results) {
        run_dialog(DialogKind::Open, request(title, &options)).await
    }

    async fn save_file(
        &self,
        _handle: ObjectPath<'_>,
        _app_id: &str,
        _parent_window: &str,
        title: &str,
        options: HashMap<String, OwnedValue>,
    ) -> (u32, Results) {
        run_dialog(DialogKind::Save, request(title, &options)).await
    }

    #[zbus(property)]
    fn version(&self) -> u32 {
        4
    }
}

const USAGE: &str = "uso: silica-portal [--address INDIRIZZO]

  --address INDIRIZZO  bus a cui collegarsi (predefinito: DBUS_SESSION_BUS_ADDRESS)";

fn main() -> ExitCode {
    let mut address = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--address" => match args.next() {
                Some(value) => address = Some(value),
                None => {
                    eprintln!("silica-portal: --address richiede un indirizzo\n{USAGE}");
                    return ExitCode::from(2);
                }
            },
            "-h" | "--help" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("silica-portal: opzione sconosciuta {other}\n{USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    match zbus::block_on(async {
        let builder = match &address {
            Some(address) => zbus::connection::Builder::address(address.as_str())?,
            None => zbus::connection::Builder::session()?,
        };
        let connection = builder
            .name(BUS_NAME)?
            .serve_at(OBJECT_PATH, FileChooser)?
            .build()
            .await?;
        eprintln!(
            "silica-portal: FileChooser su {}",
            connection
                .unique_name()
                .map_or("?".to_owned(), |n| n.to_string())
        );
        std::future::pending::<zbus::Result<()>>().await
    }) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("silica-portal: {err}");
            ExitCode::FAILURE
        }
    }
}
