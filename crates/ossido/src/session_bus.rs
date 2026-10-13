//! Il bus di sessione privato (`--session-bus`).
//!
//! Annidato in un'altra sessione (GNOME) i nomi come `org.freedesktop.Notifications` sono
//! già presi: Ossido avvia un `dbus-daemon` suo e `silica-daemon` su quel bus, e passa
//! l'indirizzo ai client che lancia (`DBUS_SESSION_BUS_ADDRESS`). I servizi di sistema
//! restano sul bus di sistema.
//!
//! Il bus non deve sopravvivere a Ossido, nemmeno se Ossido viene ucciso: `dbus-daemon`
//! gira sotto una shell che legge lo stdin collegato a Ossido e lo termina quando la pipe
//! si chiude. `silica-daemon` esce da solo quando perde il bus.

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};

use tracing::{info, warn};

/// La shell di guardia: avvia il bus, ne stampa l'indirizzo e lo chiude a fine stdin.
const WATCHDOG: &str = "dbus-daemon --session --nofork --nopidfile --print-address=1 & bus=$!
trap 'kill $bus 2>/dev/null' EXIT
read _ignored";

pub struct SessionBus {
    pub address: String,
    watchdog: Child,
    /// Tenuto aperto finché vive Ossido: chiuderlo chiude il bus.
    _guard: Option<ChildStdin>,
    daemon: Option<Child>,
}

impl SessionBus {
    /// Avvia il bus e, se si trova, `silica-daemon`.
    pub fn start() -> Result<Self, String> {
        let mut watchdog = Command::new("sh")
            .arg("-c")
            .arg(WATCHDOG)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| format!("avvio di dbus-daemon: {e}"))?;
        let guard = watchdog.stdin.take();
        let mut line = String::new();
        let read = watchdog
            .stdout
            .take()
            .map(|stdout| BufReader::new(stdout).read_line(&mut line));
        let address = line.trim().to_owned();
        if !matches!(read, Some(Ok(n)) if n > 0) || address.is_empty() {
            let _ = watchdog.kill();
            let _ = watchdog.wait();
            return Err("dbus-daemon non ha stampato l'indirizzo (pacchetto dbus?)".to_owned());
        }
        info!("bus di sessione privato: {address}");
        let mut bus = Self {
            address,
            watchdog,
            _guard: guard,
            daemon: None,
        };
        bus.daemon = bus.spawn_daemon();
        Ok(bus)
    }

    fn spawn_daemon(&self) -> Option<Child> {
        let program = daemon_path();
        match Command::new(&program)
            .args(["--address", &self.address])
            .env("DBUS_SESSION_BUS_ADDRESS", &self.address)
            .stdin(Stdio::null())
            .spawn()
        {
            Ok(child) => {
                info!("avviato {} (pid {})", program.display(), child.id());
                Some(child)
            }
            Err(err) => {
                warn!("silica-daemon non avviato ({}): {err}", program.display());
                None
            }
        }
    }
}

impl Drop for SessionBus {
    fn drop(&mut self) {
        if let Some(daemon) = &mut self.daemon {
            let _ = daemon.kill();
            let _ = daemon.wait();
        }
        // Chiudere lo stdin fa uscire la shell di guardia, che chiude il bus.
        self._guard = None;
        let _ = self.watchdog.wait();
    }
}

/// `silica-daemon` accanto all'eseguibile di Ossido (target/…), altrimenti dal PATH.
fn daemon_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("silica-daemon")))
        .filter(|path| path.is_file())
        .unwrap_or_else(|| PathBuf::from("silica-daemon"))
}
