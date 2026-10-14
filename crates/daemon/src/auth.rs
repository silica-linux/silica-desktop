//! The polkit authentication agent of the session.
//!
//! polkit asks the session's agent for a password when an action needs one
//! (`auth_admin_keep`: the time zone, the computer name, the language). The daemon registers an
//! `org.freedesktop.PolicyKit1.AuthenticationAgent` on the system bus for its logind session
//! and shows each request to the shell through `dev.frascella.Silica.Authentication1` on the
//! session bus; the shell draws the system authentication dialog and answers with the user name
//! and the password. The password is checked by polkit's own helper (`polkit-agent-helper-1`,
//! through its socket `/run/polkit/agent-helper.socket` when systemd provides it, otherwise the
//! setuid program), which runs PAM and reports the result to polkit itself: the daemon never
//! sees whether the password is right except through the helper's `SUCCESS`/`FAILURE` line.
//!
//! See `docs/daemon.md` › Authentication.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use event_listener::Event;
use futures_lite::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use futures_lite::{StreamExt, future};
use zbus::message::Header;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::{OwnedValue, Value};
use zbus::{Connection, fdo, interface};

/// The shell's interface, on the session bus.
pub const AUTH_PATH: &str = "/dev/frascella/Silica/Authentication1";
pub const AUTH_INTERFACE: &str = "dev.frascella.Silica.Authentication1";
/// The agent object polkit calls, on the system bus.
pub const AGENT_PATH: &str = "/dev/frascella/Silica/PolkitAgent";

const POLKIT: &str = "org.freedesktop.PolicyKit1";
const POLKIT_PATH: &str = "/org/freedesktop/PolicyKit1/Authority";
const POLKIT_AUTHORITY: &str = "org.freedesktop.PolicyKit1.Authority";
const LOGIND: &str = "org.freedesktop.login1";

/// How long one password check may take (PAM waits a few seconds after a wrong password).
const CHECK_TIMEOUT: Duration = Duration::from_secs(60);
/// How often a failed registration is tried again (polkit not running yet, another agent).
const RETRY: Duration = Duration::from_secs(30);
/// Requests waiting at once; more are refused (a client looping on a privileged call).
pub const MAX_REQUESTS: usize = 8;
/// Bounds on what polkit sends, before it reaches the shell.
const MAX_TEXT: usize = 1024;

/// Where the helper lives, by distribution.
const HELPERS: &[&str] = &[
    "/usr/lib/polkit-1/polkit-agent-helper-1",
    "/usr/libexec/polkit-agent-helper-1",
    "/usr/lib/policykit-1/polkit-agent-helper-1",
];
const HELPER_SOCKET: &str = "/run/polkit/agent-helper.socket";

#[derive(Clone, Debug)]
pub struct AuthConfig {
    /// The system bus (polkit); `None`: the agent is not registered.
    pub system_bus: Option<Connection>,
    /// The setuid helper, used when the socket is absent.
    pub helper: PathBuf,
    /// The socket-activated helper (polkit ≥ 126); used when it exists.
    pub helper_socket: Option<PathBuf>,
    /// The logind session to register for; `None`: `XDG_SESSION_ID`, then logind.
    pub session_id: Option<String>,
    /// The user database, for the names shown in the dialog.
    pub passwd: PathBuf,
}

impl Default for AuthConfig {
    fn default() -> Self {
        let helper = HELPERS
            .iter()
            .map(PathBuf::from)
            .find(|path| path.exists())
            .unwrap_or_else(|| PathBuf::from("/usr/lib/polkit-1/polkit-agent-helper-1"));
        Self {
            system_bus: None,
            helper,
            helper_socket: Some(PathBuf::from(HELPER_SOCKET)),
            session_id: std::env::var("XDG_SESSION_ID")
                .ok()
                .filter(|id| !id.is_empty()),
            passwd: PathBuf::from("/etc/passwd"),
        }
    }
}

/// A user who can answer a request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct User {
    pub uid: u32,
    pub name: String,
    /// The GECOS full name, or the login name.
    pub real_name: String,
}

/// The users of `/etc/passwd` with these uids, in the given order; unknown uids are skipped.
pub fn users_from_passwd(text: &str, uids: &[u32]) -> Vec<User> {
    let mut known: HashMap<u32, User> = HashMap::new();
    for line in text.lines() {
        let fields: Vec<&str> = line.split(':').collect();
        let (Some(name), Some(uid), Some(gecos)) = (fields.first(), fields.get(2), fields.get(4))
        else {
            continue;
        };
        let Ok(uid) = uid.parse::<u32>() else {
            continue;
        };
        let real = gecos.split(',').next().unwrap_or("").trim();
        known.entry(uid).or_insert_with(|| User {
            uid,
            name: (*name).to_owned(),
            real_name: if real.is_empty() {
                (*name).to_owned()
            } else {
                real.to_owned()
            },
        });
    }
    let mut users = Vec::new();
    for uid in uids {
        if let Some(user) = known.get(uid)
            && !users.iter().any(|u: &User| u.uid == *uid)
        {
            users.push(user.clone());
        }
    }
    users
}

/// The uids of polkit's identities (`unix-user`; groups arrive already expanded).
pub fn identity_uids(identities: &[(String, HashMap<String, OwnedValue>)]) -> Vec<u32> {
    identities
        .iter()
        .filter(|(kind, _)| kind == "unix-user")
        .filter_map(|(_, details)| match details.get("uid").map(|v| &**v) {
            Some(Value::U32(uid)) => Some(*uid),
            Some(Value::I32(uid)) => u32::try_from(*uid).ok(),
            _ => None,
        })
        .collect()
}

/// The user offered first: the one running the session if polkit accepts them, otherwise the
/// first identity (an administrator).
pub fn default_user(users: &[User], own_uid: Option<u32>) -> Option<&User> {
    own_uid
        .and_then(|uid| users.iter().find(|u| u.uid == uid))
        .or_else(|| users.first())
}

/// The uid of this process.
fn own_uid() -> Option<u32> {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata("/proc/self").ok().map(|m| m.uid())
}

/// Programs that run a privileged call on behalf of an app; the app is their ancestor.
const WRAPPERS: &[&str] = &[
    "timedatectl",
    "hostnamectl",
    "localectl",
    "systemctl",
    "loginctl",
    "pkexec",
    "busctl",
    "gdbus",
    "dbus-send",
    "sh",
    "bash",
    "dash",
    "zsh",
    "env",
    "flatpak-spawn",
    "systemd-run",
];

/// The app that asked: from the subject process up its parents, the first one that is not a
/// command-line wrapper (`timedatectl` run by Settings → Settings). Its executable's name, or
/// empty if none is found.
pub fn app_of(proc_dir: &Path, pid: u32) -> String {
    let mut pid = pid;
    for _ in 0..16 {
        if pid <= 1 {
            break;
        }
        let dir = proc_dir.join(pid.to_string());
        let name = std::fs::read_link(dir.join("exe"))
            .ok()
            .and_then(|exe| exe.file_name().map(|n| n.to_string_lossy().into_owned()))
            .or_else(|| {
                std::fs::read_to_string(dir.join("comm"))
                    .ok()
                    .map(|c| c.trim().to_owned())
            })
            .unwrap_or_default();
        if name.is_empty() {
            break;
        }
        if !WRAPPERS.contains(&name.as_str()) {
            return name;
        }
        // The parent: the field after the parenthesised command in `stat`.
        let Some(parent) = std::fs::read_to_string(dir.join("stat"))
            .ok()
            .and_then(|stat| {
                let rest = stat.get(stat.rfind(')')?.checked_add(1)?..)?;
                rest.split_whitespace().nth(1)?.parse::<u32>().ok()
            })
        else {
            break;
        };
        pid = parent;
    }
    String::new()
}

fn bounded(text: &str) -> String {
    text.chars().take(MAX_TEXT).collect()
}

/// How a request ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Success,
    Cancelled,
}

impl Outcome {
    fn name(self) -> &'static str {
        match self {
            Outcome::Success => "success",
            Outcome::Cancelled => "cancelled",
        }
    }
}

/// A request waiting for the user.
#[derive(Clone, Debug)]
struct Request {
    id: u32,
    cookie: String,
    action: String,
    message: String,
    icon: String,
    app: String,
    users: Vec<User>,
    default_user: String,
    /// A password check is running.
    busy: bool,
    outcome: Option<Outcome>,
}

/// The tuple of `Pending` and `Requested`: id, action id, message, icon name, app, users
/// (login name, full name), default user.
type RequestTuple = (
    u32,
    String,
    String,
    String,
    String,
    Vec<(String, String)>,
    String,
);

impl Request {
    fn tuple(&self) -> RequestTuple {
        (
            self.id,
            self.action.clone(),
            self.message.clone(),
            self.icon.clone(),
            self.app.clone(),
            self.users
                .iter()
                .map(|u| (u.name.clone(), u.real_name.clone()))
                .collect(),
            self.default_user.clone(),
        )
    }
}

#[derive(Default)]
struct Requests {
    next_id: u32,
    list: Vec<Request>,
    registered: bool,
}

#[derive(Clone)]
struct Shared {
    requests: Arc<Mutex<Requests>>,
    /// Wakes the waiting `BeginAuthentication` calls.
    changed: Arc<Event>,
    config: AuthConfig,
    session: Connection,
}

impl Shared {
    fn requests(&self) -> MutexGuard<'_, Requests> {
        self.requests.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn finish(&self, id: u32, outcome: Outcome) -> bool {
        let mut requests = self.requests();
        match requests.list.iter_mut().find(|r| r.id == id) {
            Some(request) if request.outcome.is_none() => {
                request.outcome = Some(outcome);
                drop(requests);
                self.changed.notify(usize::MAX);
                true
            }
            _ => false,
        }
    }
}

/// The agent: its object on the system bus, the shell's interface on the session bus.
pub struct AuthService {
    shared: Shared,
}

impl AuthService {
    /// Publishes `Authentication1` on `session` and the agent object on the system bus; the
    /// registration with polkit happens in `run`.
    pub async fn serve(session: Connection, config: AuthConfig) -> zbus::Result<Self> {
        let shared = Shared {
            requests: Arc::new(Mutex::new(Requests::default())),
            changed: Arc::new(Event::new()),
            config,
            session: session.clone(),
        };
        session
            .object_server()
            .at(
                AUTH_PATH,
                Authentication {
                    shared: shared.clone(),
                },
            )
            .await?;
        if let Some(system) = &shared.config.system_bus {
            system
                .object_server()
                .at(
                    AGENT_PATH,
                    Agent {
                        shared: shared.clone(),
                    },
                )
                .await?;
        }
        Ok(Self { shared })
    }

    /// Registers with polkit, and again whenever polkit restarts; never returns without a bus
    /// error.
    pub async fn run(&self) -> zbus::Result<()> {
        let Some(system) = self.shared.config.system_bus.clone() else {
            return future::pending().await;
        };
        let dbus = fdo::DBusProxy::new(&system).await?;
        let mut owners = dbus.receive_name_owner_changed().await?;
        loop {
            let registered = match register(&self.shared, &system).await {
                Ok(()) => {
                    tracing::info!("polkit agent registered");
                    true
                }
                Err(err) => {
                    tracing::warn!("polkit agent not registered: {err}");
                    false
                }
            };
            self.set_registered(registered).await;
            // polkit restarted: register again; not registered: try again later.
            let restarted = async {
                while let Some(signal) = owners.next().await {
                    if signal
                        .args()
                        .is_ok_and(|a| a.name().as_str() == POLKIT && a.new_owner().is_some())
                    {
                        return;
                    }
                }
                future::pending::<()>().await;
            };
            if registered {
                restarted.await;
            } else {
                future::or(restarted, async {
                    async_io::Timer::after(RETRY).await;
                })
                .await;
            }
        }
    }

    async fn set_registered(&self, registered: bool) {
        let changed = {
            let mut requests = self.shared.requests();
            let changed = requests.registered != registered;
            requests.registered = registered;
            changed
        };
        if !changed {
            return;
        }
        let server = self.shared.session.object_server();
        if let Ok(iface) = server.interface::<_, Authentication>(AUTH_PATH).await {
            let _ = iface
                .get()
                .await
                .registered_changed(iface.signal_emitter())
                .await;
        }
    }
}

/// The session's logind id: the configured one, otherwise logind's session of this process.
async fn session_id(shared: &Shared, system: &Connection) -> Result<String, String> {
    if let Some(id) = &shared.config.session_id {
        return Ok(id.clone());
    }
    let reply = system
        .call_method(
            Some(LOGIND),
            "/org/freedesktop/login1",
            Some("org.freedesktop.login1.Manager"),
            "GetSessionByPID",
            &(std::process::id()),
        )
        .await
        .map_err(|e| format!("logind: {e}"))?;
    let path: zbus::zvariant::OwnedObjectPath =
        reply.body().deserialize().map_err(|e| e.to_string())?;
    let session = fdo::PropertiesProxy::builder(system)
        .destination(LOGIND)
        .and_then(|b| b.path(path))
        .map_err(|e| e.to_string())?
        .build()
        .await
        .map_err(|e| e.to_string())?;
    let interface = zbus::names::InterfaceName::try_from("org.freedesktop.login1.Session")
        .map_err(|e| e.to_string())?;
    let id = session
        .get(interface, "Id")
        .await
        .map_err(|e| format!("logind: {e}"))?;
    String::try_from(id).map_err(|e| e.to_string())
}

async fn register(shared: &Shared, system: &Connection) -> Result<(), String> {
    let id = session_id(shared, system).await?;
    let subject: (&str, HashMap<&str, Value<'_>>) = (
        "unix-session",
        HashMap::from([("session-id", Value::from(id))]),
    );
    let locale = std::env::var("LC_ALL")
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| std::env::var("LANG").ok())
        .unwrap_or_else(|| "C".to_owned());
    system
        .call_method(
            Some(POLKIT),
            POLKIT_PATH,
            Some(POLKIT_AUTHORITY),
            "RegisterAuthenticationAgent",
            &(subject, locale.as_str(), AGENT_PATH),
        )
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// The helper's conversation: answers the PAM prompts and reads the verdict. `Ok(true)` on
/// `SUCCESS`, `Ok(false)` on `FAILURE`.
pub async fn converse<R, W>(
    reader: R,
    mut writer: W,
    user: &str,
    password: &str,
) -> Result<bool, String>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut lines = BufReader::new(reader).lines();
    let mut answered_password = false;
    while let Some(line) = lines.next().await {
        let line = line.map_err(|e| e.to_string())?;
        let line = line.trim_end();
        if line == "SUCCESS" {
            return Ok(true);
        }
        if line == "FAILURE" {
            return Ok(false);
        }
        let reply = if line.starts_with("PAM_PROMPT_ECHO_OFF") {
            // A second secret prompt (a one-time code): the dialog has only the password.
            if answered_password {
                return Ok(false);
            }
            answered_password = true;
            password
        } else if line.starts_with("PAM_PROMPT_ECHO_ON") {
            user
        } else {
            // PAM_ERROR_MSG, PAM_TEXT_INFO: nothing to answer.
            if !line.is_empty() {
                tracing::debug!("polkit helper: {line}");
            }
            continue;
        };
        writer
            .write_all(format!("{reply}\n").as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        writer.flush().await.map_err(|e| e.to_string())?;
    }
    Err("the polkit helper ended without an answer".to_owned())
}

/// Checks `password` for `user` against the request `cookie` through polkit's helper.
async fn check_password(
    config: &AuthConfig,
    user: &str,
    cookie: &str,
    password: &str,
) -> Result<bool, String> {
    let check = async {
        if let Some(socket) = config.helper_socket.as_deref().filter(|s| s.exists()) {
            let stream = async_io::Async::<std::os::unix::net::UnixStream>::connect(socket)
                .await
                .map_err(|e| format!("{}: {e}", socket.display()))?;
            let mut writer = &stream;
            writer
                .write_all(format!("{user}\n{cookie}\n").as_bytes())
                .await
                .map_err(|e| e.to_string())?;
            return converse(&stream, &stream, user, password).await;
        }
        let mut child = async_process::Command::new(&config.helper)
            .arg(user)
            .stdin(async_process::Stdio::piped())
            .stdout(async_process::Stdio::piped())
            .stderr(async_process::Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("{}: {e}", config.helper.display()))?;
        let (Some(mut stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            return Err("polkit helper without pipes".to_owned());
        };
        stdin
            .write_all(format!("{cookie}\n").as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        let verdict = converse(stdout, stdin, user, password).await;
        let _ = child.status().await;
        verdict
    };
    future::or(check, async {
        async_io::Timer::after(CHECK_TIMEOUT).await;
        Err("the polkit helper did not answer".to_owned())
    })
    .await
}

/// `org.freedesktop.PolicyKit1.AuthenticationAgent`, called by polkit on the system bus.
struct Agent {
    shared: Shared,
}

/// The errors polkit expects from an agent: `Cancelled` when the user dismissed the dialog.
#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.freedesktop.PolicyKit1.Error")]
pub enum PolkitError {
    #[zbus(error)]
    ZBus(zbus::Error),
    Failed(String),
    Cancelled(String),
    NotAuthorized(String),
}

impl From<fdo::Error> for PolkitError {
    fn from(error: fdo::Error) -> Self {
        PolkitError::Failed(error.to_string())
    }
}

#[interface(name = "org.freedesktop.PolicyKit1.AuthenticationAgent")]
impl Agent {
    /// polkit wants the user to authenticate for `action_id`. Returns when the user answered
    /// (the helper already told polkit) or cancelled.
    #[allow(clippy::too_many_arguments)]
    async fn begin_authentication(
        &self,
        #[zbus(header)] header: Header<'_>,
        #[zbus(connection)] system: &Connection,
        action_id: String,
        message: String,
        icon_name: String,
        details: HashMap<String, String>,
        cookie: String,
        identities: Vec<(String, HashMap<String, OwnedValue>)>,
    ) -> Result<(), PolkitError> {
        // Only polkit itself may open the dialog.
        let polkit = zbus::names::BusName::try_from(POLKIT).map_err(zbus::Error::from)?;
        let owner = fdo::DBusProxy::new(system)
            .await?
            .get_name_owner(polkit)
            .await?;
        if header.sender().map(|s| s.as_str()) != Some(owner.as_str()) {
            return Err(PolkitError::NotAuthorized("only polkit may ask".into()));
        }
        let passwd = std::fs::read_to_string(&self.shared.config.passwd).unwrap_or_default();
        let users = users_from_passwd(&passwd, &identity_uids(&identities));
        let Some(default) = default_user(&users, own_uid()).map(|u| u.name.clone()) else {
            return Err(PolkitError::Failed("no user can authenticate".into()));
        };
        let app = details
            .get("polkit.subject-pid")
            .and_then(|pid| pid.parse::<u32>().ok())
            .map(|pid| app_of(Path::new("/proc"), pid))
            .unwrap_or_default();
        let request = {
            let mut requests = self.shared.requests();
            if requests.list.len() >= MAX_REQUESTS {
                return Err(PolkitError::Failed("too many requests".into()));
            }
            requests.next_id = requests.next_id.wrapping_add(1).max(1);
            let request = Request {
                id: requests.next_id,
                cookie: bounded(&cookie),
                action: bounded(&action_id),
                message: bounded(&message),
                icon: bounded(&icon_name),
                app: bounded(&app),
                users,
                default_user: default,
                busy: false,
                outcome: None,
            };
            requests.list.push(request.clone());
            request
        };
        let id = request.id;
        let emitter = SignalEmitter::new(&self.shared.session, AUTH_PATH)?;
        Authentication::requested(&emitter, request.tuple()).await?;
        let outcome = loop {
            let listener = self.shared.changed.listen();
            let outcome = self
                .shared
                .requests()
                .list
                .iter()
                .find(|r| r.id == id)
                .and_then(|r| r.outcome);
            if let Some(outcome) = outcome {
                break outcome;
            }
            listener.await;
        };
        self.shared.requests().list.retain(|r| r.id != id);
        Authentication::closed(&emitter, id, outcome.name()).await?;
        match outcome {
            Outcome::Success => Ok(()),
            Outcome::Cancelled => Err(PolkitError::Cancelled("cancelled".into())),
        }
    }

    /// polkit no longer needs the answer (the caller went away, or it timed out).
    fn cancel_authentication(&self, cookie: String) {
        let id = self
            .shared
            .requests()
            .list
            .iter()
            .find(|r| r.cookie == cookie)
            .map(|r| r.id);
        if let Some(id) = id {
            self.shared.finish(id, Outcome::Cancelled);
        }
    }
}

/// `dev.frascella.Silica.Authentication1`: the requests, for the shell's dialog.
struct Authentication {
    shared: Shared,
}

#[interface(name = "dev.frascella.Silica.Authentication1")]
impl Authentication {
    /// The requests waiting, oldest first (a shell started later shows them).
    fn pending(&self) -> Vec<RequestTuple> {
        self.shared
            .requests()
            .list
            .iter()
            .filter(|r| r.outcome.is_none())
            .map(Request::tuple)
            .collect()
    }

    /// The user's answer. True: right, the request closes with `success`; false: wrong, the
    /// dialog stays for another try. `user` must be one of the request's users.
    async fn respond(&self, id: u32, user: String, password: String) -> fdo::Result<bool> {
        let cookie = {
            let mut requests = self.shared.requests();
            let request = requests
                .list
                .iter_mut()
                .find(|r| r.id == id && r.outcome.is_none())
                .ok_or_else(|| fdo::Error::InvalidArgs(format!("no request {id}")))?;
            if !request.users.iter().any(|u| u.name == user) {
                return Err(fdo::Error::InvalidArgs(format!(
                    "{user} cannot authenticate this request"
                )));
            }
            if request.busy {
                return Err(fdo::Error::Failed("a check is already running".into()));
            }
            request.busy = true;
            request.cookie.clone()
        };
        let verdict = check_password(&self.shared.config, &user, &cookie, &password).await;
        drop(password);
        if let Some(request) = self.shared.requests().list.iter_mut().find(|r| r.id == id) {
            request.busy = false;
        }
        match verdict {
            Ok(true) => {
                self.shared.finish(id, Outcome::Success);
                Ok(true)
            }
            Ok(false) => Ok(false),
            Err(err) => {
                tracing::warn!("polkit helper: {err}");
                Err(fdo::Error::Failed(err))
            }
        }
    }

    /// The user cancelled the dialog.
    fn cancel(&self, id: u32) -> fdo::Result<()> {
        if self.shared.finish(id, Outcome::Cancelled) {
            Ok(())
        } else {
            Err(fdo::Error::InvalidArgs(format!("no request {id}")))
        }
    }

    /// The agent is registered with polkit: privileged settings can ask for a password.
    #[zbus(property)]
    fn registered(&self) -> bool {
        self.shared.requests().registered
    }

    /// A new request.
    #[zbus(signal)]
    async fn requested(emitter: &SignalEmitter<'_>, request: RequestTuple) -> zbus::Result<()>;

    /// A request ended: `success` or `cancelled` (by the user or by polkit).
    #[zbus(signal)]
    async fn closed(emitter: &SignalEmitter<'_>, id: u32, outcome: &str) -> zbus::Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    const PASSWD: &str = "root:x:0:0:root:/root:/bin/bash\n\
        anna:x:1000:1000:Anna Rossi,,,:/home/anna:/bin/bash\n\
        bob:x:1001:1001::/home/bob:/bin/sh\n\
        broken line\n";

    #[test]
    fn users_come_from_passwd_in_identity_order() {
        let users = users_from_passwd(PASSWD, &[1001, 1000, 1000, 42]);
        assert_eq!(
            users,
            [
                User {
                    uid: 1001,
                    name: "bob".into(),
                    real_name: "bob".into()
                },
                User {
                    uid: 1000,
                    name: "anna".into(),
                    real_name: "Anna Rossi".into()
                },
            ]
        );
        assert_eq!(default_user(&users, Some(1000)).unwrap().name, "anna");
        assert_eq!(default_user(&users, Some(7)).unwrap().name, "bob");
        assert!(default_user(&[], Some(7)).is_none());
    }

    #[test]
    fn identities_keep_unix_users() {
        let uid = |v: Value<'static>| HashMap::from([("uid".to_owned(), v.try_into().unwrap())]);
        let identities = vec![
            ("unix-user".to_owned(), uid(Value::U32(1000))),
            ("unix-group".to_owned(), uid(Value::U32(10))),
            ("unix-user".to_owned(), uid(Value::I32(0))),
            ("unix-user".to_owned(), uid(Value::I32(-1))),
        ];
        assert_eq!(identity_uids(&identities), [1000, 0]);
    }

    #[test]
    fn the_app_is_the_first_non_wrapper_ancestor() {
        let dir = std::env::temp_dir().join(format!("silica-auth-proc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let process = |pid: u32, comm: &str, ppid: u32| {
            let p = dir.join(pid.to_string());
            std::fs::create_dir_all(&p).unwrap();
            std::fs::write(p.join("comm"), format!("{comm}\n")).unwrap();
            std::fs::write(p.join("stat"), format!("{pid} ({comm} (x)) S {ppid} 1 1 0")).unwrap();
        };
        process(30, "timedatectl", 20);
        process(20, "sh", 10);
        process(10, "settings", 1);
        assert_eq!(app_of(&dir, 30), "settings");
        process(40, "bash", 1);
        assert_eq!(app_of(&dir, 40), "");
        assert_eq!(app_of(&dir, 99), "");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn run_conversation(
        script: &str,
        user: &str,
        password: &str,
    ) -> (Result<bool, String>, String) {
        let mut answers = Vec::new();
        let verdict = zbus::block_on(converse(
            script.as_bytes(),
            futures_lite::io::Cursor::new(&mut answers),
            user,
            password,
        ));
        (verdict, String::from_utf8(answers).unwrap())
    }

    #[test]
    fn the_conversation_answers_prompts() {
        let (verdict, answers) = run_conversation(
            "PAM_TEXT_INFO hello\nPAM_PROMPT_ECHO_ON login:\nPAM_PROMPT_ECHO_OFF Password: \nSUCCESS\n",
            "anna",
            "secret",
        );
        assert_eq!(verdict, Ok(true));
        assert_eq!(answers, "anna\nsecret\n");
        let (verdict, _) = run_conversation("PAM_PROMPT_ECHO_OFF P\nFAILURE\n", "anna", "x");
        assert_eq!(verdict, Ok(false));
        // A second secret prompt cannot be answered from the dialog.
        let (verdict, answers) = run_conversation(
            "PAM_PROMPT_ECHO_OFF P\nPAM_PROMPT_ECHO_OFF Code\n",
            "a",
            "x",
        );
        assert_eq!(verdict, Ok(false));
        assert_eq!(answers, "x\n");
        let (verdict, _) = run_conversation("PAM_ERROR_MSG oops\n", "a", "x");
        assert!(verdict.is_err());
    }
}
