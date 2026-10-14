//! The private session bus (`--session-bus`).
//!
//! Nested in another session (GNOME) the names like `org.freedesktop.Notifications` are
//! already taken: Ossido starts its own `dbus-daemon` and `silica-daemon` on that bus, and
//! passes the address to the clients it launches (`DBUS_SESSION_BUS_ADDRESS`). The system
//! services stay on the system bus.
//!
//! The bus must not outlive Ossido, not even if Ossido is killed: `dbus-daemon` runs under a
//! shell that reads the stdin connected to Ossido and terminates it when the pipe closes.
//! `silica-daemon` exits by itself when it loses the bus.

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};

use tracing::{info, warn};

/// The guard shell: starts the bus, prints its address and closes it at the end of stdin.
const WATCHDOG: &str = "dbus-daemon --session --nofork --nopidfile --print-address=1 & bus=$!
trap 'kill $bus 2>/dev/null' EXIT
read _ignored";

pub struct SessionBus {
    pub address: String,
    watchdog: Child,
    /// Kept open while Ossido lives: closing it closes the bus.
    _guard: Option<ChildStdin>,
    daemon: Option<Child>,
    /// `silica-portal` (FileChooser and Settings), if found.
    portal: Option<Child>,
}

impl SessionBus {
    /// Starts the bus and, if found, `silica-daemon`.
    pub fn start() -> Result<Self, String> {
        let mut watchdog = Command::new("sh")
            .arg("-c")
            .arg(WATCHDOG)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| format!("starting dbus-daemon: {e}"))?;
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
            return Err("dbus-daemon did not print the address (package dbus?)".to_owned());
        }
        info!("private session bus: {address}");
        let mut bus = Self {
            address,
            watchdog,
            _guard: guard,
            daemon: None,
            portal: None,
        };
        bus.daemon = bus.spawn("silica-daemon");
        bus.portal = bus.spawn("silica-portal");
        Ok(bus)
    }

    /// Starts a Silica service on this bus (next to Ossido or from PATH).
    fn spawn(&self, name: &str) -> Option<Child> {
        let program = sibling_path(name);
        match Command::new(&program)
            .args(["--address", &self.address])
            .env("DBUS_SESSION_BUS_ADDRESS", &self.address)
            .stdin(Stdio::null())
            .spawn()
        {
            Ok(child) => {
                info!("started {} (pid {})", program.display(), child.id());
                Some(child)
            }
            Err(err) => {
                warn!("{name} not started ({}): {err}", program.display());
                None
            }
        }
    }
}

impl Drop for SessionBus {
    fn drop(&mut self) {
        for child in [&mut self.daemon, &mut self.portal].into_iter().flatten() {
            let _ = child.kill();
            let _ = child.wait();
        }
        // Closing stdin makes the guard shell exit, which closes the bus.
        self._guard = None;
        let _ = self.watchdog.wait();
    }
}

/// A Silica program next to Ossido's executable (target/…), otherwise from PATH.
fn sibling_path(name: &str) -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(name)))
        .filter(|path| path.is_file())
        .unwrap_or_else(|| PathBuf::from(name))
}
