//! Pseudo-terminals for apps, channel `silica/pty` (JSONMethodCodec): an app such as Terminal
//! runs a shell on a pty without native code of its own.
//!
//! From the app:
//! - `spawn {command?: [argv…], cwd?, env?: {k: v}, rows, cols}` → `{id}`: without `command`
//!   the user's login shell (`$SHELL`, else the passwd entry, else `/bin/sh`) as a login
//!   shell; `TERM=xterm-256color`, `COLORTERM=truecolor`, then `env`;
//! - `write {id, data: base64}`, `resize {id, rows, cols, pixelWidth?, pixelHeight?}`,
//!   `signal {id, signal: "INT"|"TERM"|"HUP"|"KILL"}`, `close {id}` (SIGHUP, then reaped) → null.
//!
//! To the app (calls on the same channel):
//! - `output {id, data: base64}`: coalesced, at most [`MAX_MESSAGE`] bytes and one message per
//!   frame per pty;
//! - `exit {id, code?, signal?}` once the child has exited and its output is delivered.
//!
//! With `spawn {grid: true}` the output is not sent as bytes: a parser thread emulates the
//! terminal (`termgrid.rs`) and the app gets `grid {…}` with the changed rows, at most once
//! per frame.
//!
//! Input is hostile: sizes are bounded, ids checked, a full output buffer pauses reading
//! (backpressure) and a full write queue refuses the write.

use std::collections::{BTreeMap, VecDeque};
use std::ffi::OsStr;
use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use rustix::fs::{Mode, OFlags};
use rustix::io::Errno;
use rustix::process::{Pid, Signal};
use rustix::pty::OpenptFlags;
use rustix::termios::Winsize;
use serde_json::{Value, json};

pub const CHANNEL: &str = "silica/pty";

/// The most output bytes in one `output` message.
pub const MAX_MESSAGE: usize = 64 * 1024;
/// Output waiting for the app beyond this pauses reading from the pty.
const MAX_PENDING: usize = 1024 * 1024;
/// Input waiting for the pty beyond this refuses `write`.
const MAX_WRITE_QUEUE: usize = 4 * 1024 * 1024;
/// At most this many ptys per window.
const MAX_PTYS: usize = 64;
const MAX_ARGS: usize = 256;
const MAX_ENV: usize = 256;
const MAX_STRING: usize = 4096;
/// How long a closed child has after SIGHUP before SIGKILL.
const CLOSE_GRACE: Duration = Duration::from_secs(2);
/// How often a running child is checked for exit while its output is still open.
const EXIT_POLL: Duration = Duration::from_millis(250);

/// A `spawn` request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnArgs {
    pub command: Option<Vec<String>>,
    pub cwd: Option<String>,
    pub env: Vec<(String, String)>,
    pub rows: u16,
    pub cols: u16,
    /// Emulate the terminal here and send rows (`grid`), not bytes (`output`).
    pub grid: bool,
}

/// A call from the app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Call {
    Spawn(SpawnArgs),
    Write {
        id: u32,
        data: Vec<u8>,
    },
    Resize {
        id: u32,
        rows: u16,
        cols: u16,
        pixel_width: u16,
        pixel_height: u16,
    },
    Signal {
        id: u32,
        signal: Signal,
    },
    Close {
        id: u32,
    },
}

fn field<'a>(args: &'a Value, key: &str) -> Option<&'a Value> {
    args.get(key).filter(|v| !v.is_null())
}

fn id_of(args: &Value) -> Result<u32, String> {
    field(args, "id")
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .ok_or_else(|| "missing or invalid id".to_owned())
}

/// A terminal dimension: 1…1000 cells, or 0…65535 pixels.
fn dimension(args: &Value, key: &str, cells: bool) -> Result<u16, String> {
    let value = field(args, key).and_then(Value::as_u64);
    match (value, cells) {
        (Some(v), true) if (1..=1000).contains(&v) => Ok(u16::try_from(v).unwrap_or(1)),
        (Some(v), false) => Ok(u16::try_from(v).unwrap_or(u16::MAX)),
        (None, false) => Ok(0),
        _ => Err(format!("missing or invalid {key}")),
    }
}

fn bounded_string(value: &Value) -> Option<String> {
    value
        .as_str()
        .filter(|s| !s.is_empty() && s.len() <= MAX_STRING && !s.contains('\0'))
        .map(str::to_owned)
}

/// Reads a call; errors name what is wrong, for the app's PlatformException.
pub fn parse_call(data: &[u8]) -> Result<Call, String> {
    let call: Value = serde_json::from_slice(data).map_err(|_| "invalid JSON".to_owned())?;
    let method = call
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let args = call.get("args").cloned().unwrap_or(Value::Null);
    match method {
        "spawn" => {
            let command = match field(&args, "command") {
                None => None,
                Some(list) => {
                    let list = list.as_array().ok_or("command must be a list")?;
                    if list.is_empty() || list.len() > MAX_ARGS {
                        return Err("command must have 1 to 256 arguments".to_owned());
                    }
                    let argv: Option<Vec<String>> = list
                        .iter()
                        .map(|a| {
                            a.as_str()
                                .filter(|s| s.len() <= MAX_STRING && !s.contains('\0'))
                                .map(str::to_owned)
                        })
                        .collect();
                    let argv = argv.ok_or("invalid argument in command")?;
                    if argv.first().is_none_or(String::is_empty) {
                        return Err("empty program".to_owned());
                    }
                    Some(argv)
                }
            };
            let cwd = match field(&args, "cwd") {
                None => None,
                Some(v) => Some(bounded_string(v).ok_or("invalid cwd")?),
            };
            let mut env = Vec::new();
            if let Some(map) = field(&args, "env") {
                let map = map.as_object().ok_or("env must be an object")?;
                if map.len() > MAX_ENV {
                    return Err("too many environment variables".to_owned());
                }
                for (key, value) in map {
                    let valid_key =
                        !key.is_empty() && key.len() <= MAX_STRING && !key.contains(['=', '\0']);
                    let value = value
                        .as_str()
                        .filter(|v| v.len() <= MAX_STRING && !v.contains('\0'));
                    match (valid_key, value) {
                        (true, Some(value)) => env.push((key.clone(), value.to_owned())),
                        _ => return Err(format!("invalid environment variable {key:?}")),
                    }
                }
            }
            Ok(Call::Spawn(SpawnArgs {
                command,
                cwd,
                env,
                rows: dimension(&args, "rows", true)?,
                cols: dimension(&args, "cols", true)?,
                grid: field(&args, "grid")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            }))
        }
        "write" => {
            let id = id_of(&args)?;
            let text = field(&args, "data")
                .and_then(Value::as_str)
                .ok_or("missing data")?;
            if text.len() > MAX_WRITE_QUEUE * 4 / 3 + 4 {
                return Err("data too large".to_owned());
            }
            let data = crate::drag::base64_decode(text).ok_or("data is not base64")?;
            Ok(Call::Write { id, data })
        }
        "resize" => Ok(Call::Resize {
            id: id_of(&args)?,
            rows: dimension(&args, "rows", true)?,
            cols: dimension(&args, "cols", true)?,
            pixel_width: dimension(&args, "pixelWidth", false)?,
            pixel_height: dimension(&args, "pixelHeight", false)?,
        }),
        "signal" => {
            let id = id_of(&args)?;
            let signal = match field(&args, "signal").and_then(Value::as_str) {
                Some("INT") => Signal::INT,
                Some("TERM") => Signal::TERM,
                Some("HUP") => Signal::HUP,
                Some("KILL") => Signal::KILL,
                _ => return Err("signal must be INT, TERM, HUP or KILL".to_owned()),
            };
            Ok(Call::Signal { id, signal })
        }
        "close" => Ok(Call::Close { id: id_of(&args)? }),
        other => Err(format!("unknown method {other:?}")),
    }
}

/// Standard base64 with padding.
pub fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let symbol = |v: u32| char::from(TABLE.get((v & 63) as usize).copied().unwrap_or(b'A'));
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = |i: usize| u32::from(chunk.get(i).copied().unwrap_or(0));
        let n = (b(0) << 16) | (b(1) << 8) | b(2);
        out.push(symbol(n >> 18));
        out.push(symbol(n >> 12));
        out.push(if chunk.len() > 1 { symbol(n >> 6) } else { '=' });
        out.push(if chunk.len() > 2 { symbol(n) } else { '=' });
    }
    out
}

/// A call from the embedder to the app.
pub fn message(method: &str, args: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({ "method": method, "args": args })).unwrap_or_default()
}

/// The user's login shell: `$SHELL`, else the passwd entry of this user, else `/bin/sh`.
fn login_shell() -> String {
    let usable = |p: &str| Path::new(p).is_absolute() && Path::new(p).exists();
    if let Ok(shell) = std::env::var("SHELL")
        && usable(&shell)
    {
        return shell;
    }
    let uid = rustix::process::getuid().as_raw().to_string();
    if let Ok(passwd) = std::fs::read_to_string("/etc/passwd") {
        for line in passwd.lines() {
            let fields: Vec<&str> = line.split(':').collect();
            if fields.get(2) == Some(&uid.as_str())
                && let Some(shell) = fields.get(6)
                && usable(shell)
            {
                return (*shell).to_owned();
            }
        }
    }
    "/bin/sh".to_owned()
}

/// What a read from the master gave.
#[derive(Debug, PartialEq, Eq)]
pub enum Read {
    Data(usize),
    WouldBlock,
    /// The slave side is closed: every process of the terminal is gone.
    Closed,
}

/// A child process on a pseudo-terminal.
pub struct Pty {
    master: OwnedFd,
    child: Child,
}

impl Pty {
    pub fn spawn(args: &SpawnArgs) -> io::Result<Self> {
        let master = rustix::pty::openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY)?;
        rustix::io::fcntl_setfd(&master, rustix::io::FdFlags::CLOEXEC)?;
        rustix::pty::grantpt(&master)?;
        rustix::pty::unlockpt(&master)?;
        let name = rustix::pty::ptsname(&master, Vec::new())?;
        let slave = rustix::fs::open(
            name.as_c_str(),
            OFlags::RDWR | OFlags::NOCTTY | OFlags::CLOEXEC,
            Mode::empty(),
        )?;
        rustix::termios::tcsetwinsize(
            &slave,
            Winsize {
                ws_row: args.rows,
                ws_col: args.cols,
                ws_xpixel: 0,
                ws_ypixel: 0,
            },
        )?;

        let mut command = match &args.command {
            Some(argv) => {
                let mut command = Command::new(argv.first().map_or("", String::as_str));
                command.args(argv.iter().skip(1));
                command
            }
            None => {
                let shell = login_shell();
                let name = Path::new(&shell)
                    .file_name()
                    .map_or(OsStr::new("sh"), |n| n);
                let mut argv0 = b"-".to_vec();
                argv0.extend_from_slice(name.as_bytes());
                let mut command = Command::new(&shell);
                command.arg0(OsStr::from_bytes(&argv0));
                command
            }
        };
        let home = std::env::var("HOME").ok();
        if let Some(dir) = args.cwd.as_deref().or(home.as_deref()) {
            command.current_dir(dir);
        }
        command
            .env("TERM", "xterm-256color")
            .env("COLORTERM", "truecolor")
            .envs(args.env.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::from(slave.try_clone()?))
            .stdout(Stdio::from(slave.try_clone()?))
            .stderr(Stdio::from(slave));
        crate::ffi::pty::controlling_terminal(&mut command);
        let child = command.spawn()?;
        rustix::io::ioctl_fionbio(&master, true)?;
        Ok(Self { master, child })
    }

    pub fn master(&self) -> BorrowedFd<'_> {
        self.master.as_fd()
    }

    pub fn read(&self, buf: &mut [u8]) -> io::Result<Read> {
        loop {
            return match rustix::io::read(&self.master, &mut *buf) {
                Ok(0) => Ok(Read::Closed),
                Ok(n) => Ok(Read::Data(n)),
                Err(Errno::INTR) => continue,
                Err(Errno::AGAIN) => Ok(Read::WouldBlock),
                // Linux answers EIO once the last slave descriptor is closed.
                Err(Errno::IO) => Ok(Read::Closed),
                Err(e) => Err(e.into()),
            };
        }
    }

    /// Writes what the pty takes now; 0 when it is full.
    pub fn write(&self, data: &[u8]) -> io::Result<usize> {
        loop {
            return match rustix::io::write(&self.master, data) {
                Ok(n) => Ok(n),
                Err(Errno::INTR) => continue,
                Err(Errno::AGAIN) => Ok(0),
                Err(e) => Err(e.into()),
            };
        }
    }

    pub fn resize(&self, rows: u16, cols: u16, pixel_width: u16, pixel_height: u16) {
        let size = Winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: pixel_width,
            ws_ypixel: pixel_height,
        };
        if let Err(err) = rustix::termios::tcsetwinsize(&self.master, size) {
            tracing::warn!("pty resize: {err}");
        }
    }

    pub fn signal(&self, signal: Signal) {
        if let Err(err) = rustix::process::kill_process(Pid::from_child(&self.child), signal) {
            tracing::debug!("pty signal: {err}");
        }
    }

    /// The exit status if the child has exited (it is then reaped).
    pub fn try_exit(&mut self) -> Option<ExitStatus> {
        self.child.try_wait().ok().flatten()
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        if self.try_exit().is_none() {
            self.signal(Signal::HUP);
            // Reaped now if it went at once; otherwise init reaps it when we exit.
            let _ = self.try_exit();
        }
    }
}

struct Session {
    pty: Pty,
    /// Output read but not yet sent to the app.
    pending: Vec<u8>,
    /// Input waiting for the pty.
    writes: VecDeque<u8>,
    /// The slave side is closed: nothing more to read.
    eof: bool,
    /// Reading stopped because `pending` is full.
    paused: bool,
    /// Closed by the app: not reported, killed if it ignores SIGHUP.
    closing: Option<Instant>,
    last_exit_poll: Instant,
    /// The emulator, for `grid` sessions.
    grid: Option<crate::termgrid::GridWorker>,
}

/// The ptys of a window.
#[derive(Default)]
pub struct PtyHost {
    sessions: BTreeMap<u32, Session>,
    next_id: u32,
}

/// What the event loop must do after a read.
#[derive(Debug, PartialEq, Eq)]
pub enum Readable {
    /// Keep the source.
    Continue,
    /// Disable the source until [`Tick::resume`] names it.
    Pause,
    /// Remove the source: the pty is closed.
    Remove,
}

/// What a frame produced.
#[derive(Debug, Default)]
pub struct Tick {
    /// Calls for the app.
    pub messages: Vec<Vec<u8>>,
    /// Paused ptys that can be read again.
    pub resume: Vec<u32>,
    /// Ptys that are gone (their source must be removed).
    pub gone: Vec<u32>,
    /// Nothing left to do: the frame timer can stop.
    pub idle: bool,
}

/// The reply to a call: a JSON value, and for `spawn` the new pty to watch.
#[derive(Debug)]
pub struct Reply {
    pub value: Value,
    pub spawned: Option<u32>,
    /// A pty closed by the app: its source must be removed.
    pub closed: Option<u32>,
}

impl PtyHost {
    pub fn handle(&mut self, call: Call) -> Result<Reply, String> {
        let done = |value| Reply {
            value,
            spawned: None,
            closed: None,
        };
        match call {
            Call::Spawn(args) => {
                if self.sessions.len() >= MAX_PTYS {
                    return Err("too many terminals".to_owned());
                }
                let pty = Pty::spawn(&args).map_err(|e| format!("spawn: {e}"))?;
                let grid = match args.grid {
                    true => Some(
                        crate::termgrid::GridWorker::new(args.cols, args.rows)
                            .map_err(|e| format!("terminal thread: {e}"))?,
                    ),
                    false => None,
                };
                let id = self.next_id;
                self.next_id = self.next_id.wrapping_add(1);
                self.sessions.insert(
                    id,
                    Session {
                        pty,
                        pending: Vec::new(),
                        writes: VecDeque::new(),
                        eof: false,
                        paused: false,
                        closing: None,
                        last_exit_poll: Instant::now(),
                        grid,
                    },
                );
                Ok(Reply {
                    value: json!({ "id": id }),
                    spawned: Some(id),
                    closed: None,
                })
            }
            Call::Write { id, data } => {
                let session = self.live(id)?;
                if session.writes.len().saturating_add(data.len()) > MAX_WRITE_QUEUE {
                    return Err("write queue full".to_owned());
                }
                session.writes.extend(data);
                flush_writes(session);
                Ok(done(Value::Null))
            }
            Call::Resize {
                id,
                rows,
                cols,
                pixel_width,
                pixel_height,
            } => {
                let session = self.live(id)?;
                session.pty.resize(rows, cols, pixel_width, pixel_height);
                if let Some(grid) = &session.grid {
                    grid.resize(cols, rows);
                }
                Ok(done(Value::Null))
            }
            Call::Signal { id, signal } => {
                self.live(id)?.pty.signal(signal);
                Ok(done(Value::Null))
            }
            Call::Close { id } => {
                let session = self.live(id)?;
                session.pty.signal(Signal::HUP);
                session.closing = Some(Instant::now());
                session.pending.clear();
                session.writes.clear();
                Ok(Reply {
                    value: Value::Null,
                    spawned: None,
                    closed: Some(id),
                })
            }
        }
    }

    fn live(&mut self, id: u32) -> Result<&mut Session, String> {
        self.sessions
            .get_mut(&id)
            .filter(|s| s.closing.is_none())
            .ok_or_else(|| format!("no terminal {id}"))
    }

    /// The master of `id`, to watch for input.
    pub fn master(&self, id: u32) -> Option<BorrowedFd<'_>> {
        self.sessions.get(&id).map(|s| s.pty.master())
    }

    /// The pty `id` has output: read what is there, up to the pending limit.
    pub fn readable(&mut self, id: u32) -> Readable {
        let Some(session) = self.sessions.get_mut(&id) else {
            return Readable::Remove;
        };
        if session.closing.is_some() {
            return Readable::Remove;
        }
        let mut buf = [0u8; 16 * 1024];
        loop {
            if session.pending.len() >= MAX_PENDING {
                session.paused = true;
                return Readable::Pause;
            }
            match session.pty.read(&mut buf) {
                Ok(Read::Data(n)) => session
                    .pending
                    .extend_from_slice(buf.get(..n).unwrap_or(&[])),
                Ok(Read::WouldBlock) => return Readable::Continue,
                Ok(Read::Closed) => {
                    session.eof = true;
                    return Readable::Remove;
                }
                Err(err) => {
                    tracing::warn!("pty {id}: {err}");
                    session.eof = true;
                    return Readable::Remove;
                }
            }
        }
    }

    /// One frame: input to the ptys, at most one `output` per pty, exits, reaped children.
    pub fn tick(&mut self, now: Instant) -> Tick {
        let mut tick = Tick::default();
        let mut finished = Vec::new();
        for (&id, session) in &mut self.sessions {
            if let Some(since) = session.closing {
                if session.pty.try_exit().is_some() {
                    finished.push(id);
                } else if now.saturating_duration_since(since) >= CLOSE_GRACE {
                    session.pty.signal(Signal::KILL);
                }
                continue;
            }
            flush_writes(session);
            if let Some(grid) = &session.grid {
                // The parser thread takes the backlog while it keeps up; beyond MAX_PENDING
                // queued there, reading from the pty pauses (backpressure to the program).
                if !session.pending.is_empty() && grid.queued() < MAX_PENDING {
                    grid.feed(std::mem::take(&mut session.pending));
                }
                let (frame, replies) = grid.frame(id);
                if !replies.is_empty() && session.writes.len() < MAX_WRITE_QUEUE {
                    session.writes.extend(replies);
                    flush_writes(session);
                }
                if let Some(frame) = frame {
                    tick.messages.push(message("grid", frame));
                }
            } else if !session.pending.is_empty() {
                let take = session.pending.len().min(MAX_MESSAGE);
                let chunk: Vec<u8> = session.pending.drain(..take).collect();
                tick.messages.push(message(
                    "output",
                    json!({ "id": id, "data": base64_encode(&chunk) }),
                ));
            }
            if session.paused && session.pending.len() < MAX_PENDING / 2 {
                session.paused = false;
                tick.resume.push(id);
            }
            // Exit: at once after the end of the output, otherwise now and then (a
            // background job may keep the terminal open after the shell is gone).
            let poll =
                session.eof || now.saturating_duration_since(session.last_exit_poll) >= EXIT_POLL;
            if poll && session.pending.is_empty() {
                session.last_exit_poll = now;
                if let Some(status) = session.pty.try_exit() {
                    if !session.eof {
                        // Drain what the child wrote before exiting; reported next frame.
                        session.eof = true;
                        continue;
                    }
                    tick.messages.push(message("exit", exit_args(id, status)));
                    finished.push(id);
                }
            }
        }
        for id in finished {
            self.sessions.remove(&id);
            tick.gone.push(id);
        }
        tick.idle = self.sessions.is_empty();
        tick
    }
}

fn flush_writes(session: &mut Session) {
    while !session.writes.is_empty() {
        let (front, _) = session.writes.as_slices();
        match session.pty.write(front) {
            Ok(0) => break,
            Ok(n) => {
                session.writes.drain(..n.min(session.writes.len()));
            }
            Err(err) => {
                tracing::debug!("pty write: {err}");
                session.writes.clear();
            }
        }
    }
}

fn exit_args(id: u32, status: ExitStatus) -> Value {
    match (status.code(), status.signal()) {
        (Some(code), _) => json!({ "id": id, "code": code }),
        (None, Some(signal)) => json!({ "id": id, "signal": signal }),
        (None, None) => json!({ "id": id }),
    }
}

/// An error reply of the JSONMethodCodec.
pub fn error(message: &str) -> Vec<u8> {
    serde_json::to_vec(&json!(["pty", message, null])).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(method: &str, args: Value) -> Vec<u8> {
        serde_json::to_vec(&json!({ "method": method, "args": args })).unwrap()
    }

    #[test]
    fn base64_round_trip() {
        for data in [&b""[..], b"h", b"hi", b"hi!", b"\x00\xff\x10binary\r\n"] {
            let text = base64_encode(data);
            assert_eq!(crate::drag::base64_decode(&text).unwrap(), data);
        }
        assert_eq!(base64_encode(b"hello"), "aGVsbG8=");
    }

    #[test]
    fn calls_are_parsed_and_hostile_ones_refused() {
        assert_eq!(
            parse_call(&call("spawn", json!({ "rows": 24, "cols": 80 }))).unwrap(),
            Call::Spawn(SpawnArgs {
                command: None,
                cwd: None,
                env: vec![],
                rows: 24,
                cols: 80,
                grid: false,
            })
        );
        let Call::Spawn(args) = parse_call(&call(
            "spawn",
            json!({ "command": ["/bin/sh", "-c", "true"], "cwd": "/tmp",
                    "env": { "A": "1" }, "rows": 10, "cols": 20 }),
        ))
        .unwrap() else {
            panic!("not a spawn");
        };
        assert_eq!(args.command.unwrap(), ["/bin/sh", "-c", "true"]);
        assert_eq!(args.env, [("A".to_owned(), "1".to_owned())]);
        assert_eq!(
            parse_call(&call("write", json!({ "id": 3, "data": "aGk=" }))).unwrap(),
            Call::Write {
                id: 3,
                data: b"hi".to_vec()
            }
        );
        assert_eq!(
            parse_call(&call("signal", json!({ "id": 1, "signal": "INT" }))).unwrap(),
            Call::Signal {
                id: 1,
                signal: Signal::INT
            }
        );
        for bad in [
            call("spawn", json!({ "rows": 0, "cols": 80 })),
            call("spawn", json!({ "rows": 24, "cols": 100000 })),
            call("spawn", json!({ "command": [], "rows": 1, "cols": 1 })),
            call("spawn", json!({ "command": [""], "rows": 1, "cols": 1 })),
            call(
                "spawn",
                json!({ "env": { "A=B": "x" }, "rows": 1, "cols": 1 }),
            ),
            call("write", json!({ "id": -1, "data": "aGk=" })),
            call("write", json!({ "id": 1, "data": "@@" })),
            call("signal", json!({ "id": 1, "signal": "STOP" })),
            call("nope", json!({})),
            b"not json".to_vec(),
        ] {
            assert!(
                parse_call(&bad).is_err(),
                "{}",
                String::from_utf8_lossy(&bad)
            );
        }
    }

    /// Runs frames until `done` holds or two seconds pass.
    fn run_until(
        host: &mut PtyHost,
        id: u32,
        out: &mut Vec<u8>,
        done: impl Fn(&[u8], &[Value]) -> bool,
    ) -> Vec<Value> {
        let start = Instant::now();
        let mut exits = Vec::new();
        while start.elapsed() < Duration::from_secs(5) {
            let _ = host.readable(id);
            let tick = host.tick(Instant::now());
            for m in tick.messages {
                let v: Value = serde_json::from_slice(&m).unwrap();
                match v["method"].as_str().unwrap() {
                    "output" => out.extend(
                        crate::drag::base64_decode(v["args"]["data"].as_str().unwrap()).unwrap(),
                    ),
                    "exit" => exits.push(v["args"].clone()),
                    _ => {}
                }
            }
            if done(out, &exits) {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        exits
    }

    #[test]
    fn a_shell_on_a_pty_sees_the_size_and_exits() {
        let mut host = PtyHost::default();
        let spawned = host
            .handle(
                parse_call(&call(
                    "spawn",
                    json!({ "command": ["/bin/sh", "-c", "echo hi; read x; stty size; exit 3"],
                            "rows": 24, "cols": 80 }),
                ))
                .unwrap(),
            )
            .unwrap();
        let id = spawned.spawned.unwrap();
        assert_eq!(spawned.value, json!({ "id": id }));
        let mut out = Vec::new();
        run_until(&mut host, id, &mut out, |o, _| {
            String::from_utf8_lossy(o).contains("hi")
        });
        assert!(String::from_utf8_lossy(&out).contains("hi"));
        // A new size, then the line the shell waits for.
        host.handle(
            parse_call(&call(
                "resize",
                json!({ "id": id, "rows": 30, "cols": 100 }),
            ))
            .unwrap(),
        )
        .unwrap();
        host.handle(Call::Write {
            id,
            data: b"\n".to_vec(),
        })
        .unwrap();
        let exits = run_until(&mut host, id, &mut out, |_, exits| !exits.is_empty());
        let text = String::from_utf8_lossy(&out);
        assert!(text.contains("30 100"), "output: {text:?}");
        assert_eq!(exits, vec![json!({ "id": id, "code": 3 })]);
        // Gone: further calls are refused.
        assert!(
            host.handle(Call::Write {
                id,
                data: b"x".to_vec()
            })
            .is_err()
        );
    }

    #[test]
    fn a_grid_session_sends_rows_not_bytes() {
        let mut host = PtyHost::default();
        let id = host
            .handle(
                parse_call(&call(
                    "spawn",
                    json!({ "command": ["/bin/sh", "-c", "printf 'hi\\033[31mred'; sleep 1"],
                            "rows": 4, "cols": 20, "grid": true }),
                ))
                .unwrap(),
            )
            .unwrap()
            .spawned
            .unwrap();
        let start = Instant::now();
        let mut seen = None;
        while start.elapsed() < Duration::from_secs(5) && seen.is_none() {
            let _ = host.readable(id);
            for m in host.tick(Instant::now()).messages {
                let v: Value = serde_json::from_slice(&m).unwrap();
                assert_ne!(v["method"], "output", "grid sessions never send bytes");
                if v["method"] == "grid" && v["args"]["lines"].to_string().contains("red") {
                    seen = Some(v["args"].clone());
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let frame = seen.expect("a grid frame with the text");
        assert_eq!(frame["lines"][0]["runs"][0], json!(["hi", -1, -1, 0]));
        assert_eq!(
            frame["lines"][0]["runs"][1],
            json!(["red", 0xcd0000, -1, 0])
        );
    }

    #[test]
    fn close_hangs_up_and_reaps() {
        let mut host = PtyHost::default();
        let id = host
            .handle(Call::Spawn(SpawnArgs {
                command: Some(vec!["/bin/sh".into(), "-c".into(), "sleep 30".into()]),
                cwd: None,
                env: vec![],
                rows: 5,
                cols: 5,
                grid: false,
            }))
            .unwrap()
            .spawned
            .unwrap();
        let reply = host.handle(Call::Close { id }).unwrap();
        assert_eq!(reply.closed, Some(id));
        let start = Instant::now();
        let mut gone = false;
        while start.elapsed() < Duration::from_secs(5) && !gone {
            let tick = host.tick(Instant::now());
            assert!(tick.messages.is_empty(), "a closed pty reports nothing");
            gone = tick.gone.contains(&id);
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(gone);
    }
}
