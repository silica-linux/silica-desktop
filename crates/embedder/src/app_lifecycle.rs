//! The app's lifecycle, as on the reference (silica-desktop#142).
//!
//! Closing an app's last window does not end the app. Ossido hides the window
//! (`silica-window-v1` › `set_hidden`), and the engine stays alive with the app's state. A
//! click on the app in the Dock (or launching it again) shows the window at once, without a
//! cold start. Only Quit (Cmd+Q, the Dock menu) ends the app. An app that should end with
//! its last window (a single-window utility such as Calculator) says so on the `silica/app`
//! channel.
//!
//! Every app process registers itself in the app registry (silica-control `apps`): a
//! unix socket answering `status`, `reopen` and `quit`. A second launch of an app with a
//! hidden window hands over to it (`reopen`) and exits.
//!
//! A windowless app is ended by the system when it has been idle for a long time or memory
//! is short (PSI). It leaves a `.parked` marker, so the Dock keeps showing it and a click
//! starts it again. Both limits are provisional (not measured on the reference).
//!
//! The `silica/app` channel (JSONMethodCodec):
//! - from the app: `setQuitWhenLastWindowCloses {value}`, `quit`;
//! - to the app: `lastWindowClosed`, `reopen {shown}` (`shown`: the hidden window came back;
//!   false when the app still had a window on screen).

use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::time::Duration;

use silica_control::apps;

pub const CHANNEL: &str = "silica/app";

/// A windowless app idle for longer than this is ended (provisional).
pub const IDLE_LIMIT: Duration = Duration::from_secs(10 * 60);

/// Memory pressure (PSI `some avg10`, percent of time stalled) above which a windowless
/// app is ended (provisional).
pub const PRESSURE_LIMIT: f64 = 10.0;

/// How often a windowless app checks the two limits.
pub const CHECK_PERIOD: Duration = Duration::from_secs(30);

/// How long a registry client may take to send its request.
const CLIENT_TIMEOUT: Duration = Duration::from_millis(200);

/// A request on the registry socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    Status,
    Reopen,
    Quit,
}

pub fn parse_request(line: &str) -> Option<Request> {
    match line.trim() {
        "status" => Some(Request::Status),
        "reopen" => Some(Request::Reopen),
        "quit" => Some(Request::Quit),
        _ => None,
    }
}

/// This process in the app registry: the socket file goes away with it.
pub struct Registration {
    path: PathBuf,
    parked: PathBuf,
}

impl Registration {
    /// Writes the `.parked` marker: the system ends this windowless app.
    pub fn park(&self) {
        if let Err(err) = std::fs::write(&self.parked, b"") {
            tracing::warn!("app registry: {}: {err}", self.parked.display());
        }
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Registers this process of `app_id`; the listener is non-blocking, for the event loop.
pub fn register(app_id: &str) -> std::io::Result<(Registration, UnixListener)> {
    if !apps::valid_app_id(app_id) {
        return Err(std::io::Error::other(format!(
            "app id not usable: {app_id}"
        )));
    }
    let dir = apps::dir().ok_or_else(|| std::io::Error::other("XDG_RUNTIME_DIR is not set"))?;
    std::fs::create_dir_all(&dir)?;
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
    // Started again: whatever the system ended is running now.
    let parked = apps::parked_path(&dir, app_id);
    let _ = std::fs::remove_file(&parked);
    let path = apps::socket_path(&dir, app_id, std::process::id());
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path)?;
    listener.set_nonblocking(true)?;
    Ok((Registration { path, parked }, listener))
}

/// A second launch: if another process of `app_id` has its window hidden, it shows it and
/// this process has nothing to do.
pub fn forward(app_id: &str) -> bool {
    apps::valid_app_id(app_id) && apps::reopen(app_id, false)
}

/// Reads one request from a registry client (bounded, with a short timeout).
pub fn read_request(stream: &UnixStream) -> Option<Request> {
    stream.set_nonblocking(false).ok()?;
    stream.set_read_timeout(Some(CLIENT_TIMEOUT)).ok()?;
    stream.set_write_timeout(Some(CLIENT_TIMEOUT)).ok()?;
    let mut line = String::new();
    BufReader::new(std::io::Read::take(stream, apps::MAX_LINE as u64))
        .read_line(&mut line)
        .ok()?;
    parse_request(&line)
}

/// Answers a registry client with one line.
pub fn answer(mut stream: &UnixStream, text: &str) {
    let _ = stream
        .write_all(text.as_bytes())
        .and_then(|()| stream.write_all(b"\n"));
}

/// The `some avg10` value of a PSI file (`/proc/pressure/memory`).
pub fn parse_psi_some_avg10(text: &str) -> Option<f64> {
    let line = text.lines().find(|l| l.starts_with("some "))?;
    line.split_whitespace()
        .find_map(|field| field.strip_prefix("avg10="))?
        .parse()
        .ok()
}

/// The current memory pressure, if the kernel reports it.
pub fn memory_pressure() -> Option<f64> {
    parse_psi_some_avg10(&std::fs::read_to_string("/proc/pressure/memory").ok()?)
}

/// The idle limit: [`IDLE_LIMIT`], or `SILICA_APP_IDLE_LIMIT` seconds (tests, tuning).
pub fn idle_limit() -> Duration {
    std::env::var("SILICA_APP_IDLE_LIMIT")
        .ok()
        .and_then(|secs| secs.parse::<u64>().ok())
        .map_or(IDLE_LIMIT, Duration::from_secs)
}

/// How often to check: [`CHECK_PERIOD`], sooner with a short idle limit.
pub fn check_period(limit: Duration) -> Duration {
    CHECK_PERIOD.min(limit.max(Duration::from_secs(1)))
}

/// Whether a windowless app should be ended now.
pub fn should_end(hidden_for: Duration, limit: Duration, pressure: Option<f64>) -> bool {
    hidden_for >= limit || pressure.is_some_and(|p| p > PRESSURE_LIMIT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_are_parsed() {
        assert_eq!(parse_request("status\n"), Some(Request::Status));
        assert_eq!(parse_request(" reopen "), Some(Request::Reopen));
        assert_eq!(parse_request("quit"), Some(Request::Quit));
        assert_eq!(parse_request("rm -rf"), None);
    }

    #[test]
    fn psi_is_parsed() {
        let text = "some avg10=12.50 avg60=3.00 avg300=1.00 total=123\n\
                    full avg10=1.00 avg60=0.00 avg300=0.00 total=12\n";
        assert_eq!(parse_psi_some_avg10(text), Some(12.5));
        assert_eq!(parse_psi_some_avg10("nothing"), None);
    }

    #[test]
    fn idle_or_pressure_end_a_windowless_app() {
        assert!(!should_end(Duration::from_secs(60), IDLE_LIMIT, Some(0.5)));
        assert!(!should_end(Duration::from_secs(60), IDLE_LIMIT, None));
        assert!(should_end(IDLE_LIMIT, IDLE_LIMIT, None));
        assert!(should_end(Duration::from_secs(1), IDLE_LIMIT, Some(25.0)));
        assert_eq!(check_period(Duration::from_secs(5)), Duration::from_secs(5));
        assert_eq!(check_period(IDLE_LIMIT), CHECK_PERIOD);
    }
}
