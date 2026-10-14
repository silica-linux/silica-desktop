//! The FileChooser backend on a private bus: calls "Mustache" (a script in its place, from the
//! .desktop entry) and returns the chosen URIs.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use zbus::proxy;
use zbus::zvariant::{ObjectPath, OwnedValue, Value};

#[proxy(
    interface = "org.freedesktop.impl.portal.FileChooser",
    default_service = "org.freedesktop.impl.portal.desktop.silica",
    default_path = "/org/freedesktop/portal/desktop"
)]
trait FileChooser {
    fn open_file(
        &self,
        handle: ObjectPath<'_>,
        app_id: &str,
        parent_window: &str,
        title: &str,
        options: HashMap<&str, Value<'_>>,
    ) -> zbus::Result<(u32, HashMap<String, OwnedValue>)>;
}

struct Killed(Child);

impl Drop for Killed {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn open_file_runs_mustache_and_returns_its_uris() {
    let dir = std::env::temp_dir().join(format!("silica-portal-it-{}", std::process::id()));
    let apps = dir.join("applications");
    std::fs::create_dir_all(&apps).unwrap();
    // "Mustache": writes the arguments it received and chooses a file.
    let script = dir.join("mustache.sh");
    let args_file = dir.join("argomenti");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\necho 'file:///home/io/scelto.txt'\n",
            args_file.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::write(
        apps.join("dev.frascella.Mustache.desktop"),
        format!(
            "[Desktop Entry]\nType=Application\nName=Mustache\nExec={} --bundle %F\n",
            script.display()
        ),
    )
    .unwrap();

    let mut bus = Command::new("dbus-daemon")
        .args(["--session", "--nofork", "--nopidfile", "--print-address=1"])
        .stdout(Stdio::piped())
        .spawn()
        .expect("dbus-daemon (package dbus) is required for this test");
    let mut address = String::new();
    BufReader::new(bus.stdout.take().unwrap())
        .read_line(&mut address)
        .unwrap();
    let _bus = Killed(bus);
    let address = address.trim().to_owned();
    let _portal = Killed(
        Command::new(env!("CARGO_BIN_EXE_silica-portal"))
            .args(["--address", &address])
            .env("XDG_DATA_HOME", &dir)
            .env("XDG_DATA_DIRS", &dir)
            .spawn()
            .unwrap(),
    );

    zbus::block_on(async {
        let connection = zbus::connection::Builder::address(address.as_str())
            .unwrap()
            .build()
            .await
            .unwrap();
        let chooser = FileChooserProxy::new(&connection).await.unwrap();
        // The portal needs a moment to take the name.
        let mut answer = None;
        for _ in 0..50 {
            let options = HashMap::from([
                ("multiple", Value::from(true)),
                ("current_folder", Value::from(b"/home/io\0".to_vec())),
            ]);
            match chooser
                .open_file(
                    ObjectPath::try_from("/org/freedesktop/portal/desktop/request/1").unwrap(),
                    "org.esempio.App",
                    "",
                    "Scegli",
                    options,
                )
                .await
            {
                Ok(result) => {
                    answer = Some(result);
                    break;
                }
                Err(_) => async_io::Timer::after(Duration::from_millis(100)).await,
            };
        }
        let (response, results) = answer.expect("the portal does not answer");
        assert_eq!(response, 0);
        let uris: Vec<String> = results["uris"].try_clone().unwrap().try_into().unwrap();
        assert_eq!(uris, ["file:///home/io/scelto.txt"]);
    });
    let args = std::fs::read_to_string(&args_file).unwrap();
    let args: Vec<&str> = args.lines().collect();
    assert_eq!(
        args,
        [
            "--bundle",
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
    std::fs::remove_dir_all(&dir).unwrap();
}
