//! The polkit agent with a fake polkit on a private "system" bus and fake helpers: the setuid
//! program (a script) and the socket-activated one (a thread on a Unix socket).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_lite::{StreamExt, future};
use silica_daemon::auth::{AGENT_PATH, AuthConfig, AuthService};
use zbus::zvariant::{OwnedValue, Value};
use zbus::{Connection, interface, proxy};

struct Bus {
    child: Child,
    address: String,
}

impl Bus {
    fn start() -> Self {
        let mut child = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--nopidfile", "--print-address=1"])
            .stdout(Stdio::piped())
            .spawn()
            .expect("dbus-daemon (package dbus) is required for these tests");
        let mut line = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut line)
            .unwrap();
        Self {
            child,
            address: line.trim().to_owned(),
        }
    }

    async fn connect(&self) -> Connection {
        zbus::connection::Builder::address(self.address.as_str())
            .unwrap()
            .build()
            .await
            .unwrap()
    }
}

impl Drop for Bus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

type Registration = (String, HashMap<String, OwnedValue>, String, String);

/// polkit's authority: it only records the agent's registration.
struct FakeAuthority {
    registered: Arc<Mutex<Vec<Registration>>>,
}

#[interface(name = "org.freedesktop.PolicyKit1.Authority")]
impl FakeAuthority {
    fn register_authentication_agent(
        &self,
        subject: (String, HashMap<String, OwnedValue>),
        locale: String,
        object_path: String,
    ) {
        self.registered
            .lock()
            .unwrap()
            .push((subject.0, subject.1, locale, object_path));
    }
}

#[proxy(
    interface = "org.freedesktop.PolicyKit1.AuthenticationAgent",
    default_path = "/dev/frascella/Silica/PolkitAgent"
)]
trait Agent {
    fn begin_authentication(
        &self,
        action_id: &str,
        message: &str,
        icon_name: &str,
        details: HashMap<&str, &str>,
        cookie: &str,
        identities: Vec<(&str, HashMap<&str, Value<'_>>)>,
    ) -> zbus::Result<()>;
    fn cancel_authentication(&self, cookie: &str) -> zbus::Result<()>;
}

type RequestTuple = (
    u32,
    String,
    String,
    String,
    String,
    Vec<(String, String)>,
    String,
);

#[proxy(
    interface = "dev.frascella.Silica.Authentication1",
    default_path = "/dev/frascella/Silica/Authentication1"
)]
trait Authentication1 {
    fn pending(&self) -> zbus::Result<Vec<RequestTuple>>;
    fn respond(&self, id: u32, user: &str, password: &str) -> zbus::Result<bool>;
    fn cancel(&self, id: u32) -> zbus::Result<()>;
    #[zbus(property)]
    fn registered(&self) -> zbus::Result<bool>;
    #[zbus(signal)]
    fn requested(&self, request: RequestTuple) -> zbus::Result<()>;
    #[zbus(signal)]
    fn closed(&self, id: u32, outcome: String) -> zbus::Result<()>;
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("silica-auth-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn own_uid() -> u32 {
    std::fs::metadata("/proc/self").unwrap().uid()
}

/// A passwd file where this process's uid is `tester`, and uid 0 is `root`.
fn passwd(dir: &std::path::Path) -> PathBuf {
    let file = dir.join("passwd");
    std::fs::write(
        &file,
        format!(
            "root:x:0:0:root:/root:/bin/sh\ntester:x:{}:100:Test User,,,:/home/t:/bin/sh\n",
            own_uid()
        ),
    )
    .unwrap();
    file
}

/// The setuid helper's protocol: the user as argument, the cookie on stdin, a password prompt,
/// `SUCCESS` for `secret`.
fn fake_helper(dir: &std::path::Path) -> PathBuf {
    let helper = dir.join("polkit-agent-helper-1");
    std::fs::write(
        &helper,
        "#!/bin/sh\n\
         read cookie\n\
         echo \"PAM_TEXT_INFO checking $1\"\n\
         echo 'PAM_PROMPT_ECHO_OFF Password: '\n\
         read password\n\
         if [ \"$password\" = secret ] && [ \"$1\" = tester ] && [ -n \"$cookie\" ]; then\n\
           echo SUCCESS\n\
         else\n\
           echo FAILURE\n\
         fi\n",
    )
    .unwrap();
    std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o755)).unwrap();
    helper
}

fn identities(uids: &[u32]) -> Vec<(&'static str, HashMap<&'static str, Value<'static>>)> {
    uids.iter()
        .map(|uid| ("unix-user", HashMap::from([("uid", Value::U32(*uid))])))
        .collect()
}

struct Setup {
    _session: Bus,
    _system: Bus,
    service: AuthService,
    polkit: Connection,
    other: Connection,
    client: Connection,
    daemon_session: String,
    daemon_system: String,
    registered: Arc<Mutex<Vec<Registration>>>,
}

async fn setup(session: Bus, system: Bus, config: AuthConfig) -> Setup {
    let polkit = system.connect().await;
    let registered = Arc::new(Mutex::new(Vec::new()));
    polkit
        .object_server()
        .at(
            "/org/freedesktop/PolicyKit1/Authority",
            FakeAuthority {
                registered: Arc::clone(&registered),
            },
        )
        .await
        .unwrap();
    polkit
        .request_name("org.freedesktop.PolicyKit1")
        .await
        .unwrap();
    let daemon_session_conn = session.connect().await;
    let daemon_system_conn = system.connect().await;
    let daemon_session = daemon_session_conn.unique_name().unwrap().to_string();
    let daemon_system = daemon_system_conn.unique_name().unwrap().to_string();
    let service = AuthService::serve(
        daemon_session_conn,
        AuthConfig {
            system_bus: Some(daemon_system_conn),
            ..config
        },
    )
    .await
    .unwrap();
    Setup {
        other: system.connect().await,
        client: session.connect().await,
        _session: session,
        _system: system,
        service,
        polkit,
        daemon_session,
        daemon_system,
        registered,
    }
}

async fn agent<'a>(connection: &'a Connection, daemon: &'a str) -> AgentProxy<'a> {
    AgentProxy::builder(connection)
        .destination(daemon)
        .unwrap()
        .build()
        .await
        .unwrap()
}

#[test]
fn the_agent_registers_and_asks_the_shell() {
    let dir = temp_dir("helper");
    let config = AuthConfig {
        system_bus: None,
        helper: fake_helper(&dir),
        helper_socket: Some(dir.join("no-socket")),
        session_id: Some("7".to_owned()),
        passwd: passwd(&dir),
    };
    zbus::block_on(async {
        let s = setup(Bus::start(), Bus::start(), config).await;
        let test = async {
            let shell = Authentication1Proxy::builder(&s.client)
                .destination(s.daemon_session.as_str())
                .unwrap()
                .build()
                .await
                .unwrap();
            let mut requested = shell.receive_requested().await.unwrap();
            let mut closed = shell.receive_closed().await.unwrap();

            // Registered for the session, at the agent's path.
            for _ in 0..100 {
                if !s.registered.lock().unwrap().is_empty() {
                    break;
                }
                async_io::Timer::after(Duration::from_millis(20)).await;
            }
            let (kind, details, _locale, path) = s.registered.lock().unwrap()[0].clone();
            assert_eq!(kind, "unix-session");
            assert_eq!(
                String::try_from(details["session-id"].try_clone().unwrap()).unwrap(),
                "7"
            );
            assert_eq!(path, AGENT_PATH);
            let mut registered = false;
            for _ in 0..100 {
                registered = shell.registered().await.unwrap();
                if registered {
                    break;
                }
                async_io::Timer::after(Duration::from_millis(20)).await;
            }
            assert!(registered);

            let polkit = agent(&s.polkit, &s.daemon_system).await;
            // A wrong password, then the right one.
            let begin = polkit.begin_authentication(
                "org.freedesktop.timedate1.set-timezone",
                "Authentication is required to set the time zone.",
                "",
                HashMap::new(),
                "cookie-1",
                identities(&[0, own_uid()]),
            );
            let answer = async {
                let request = requested.next().await.unwrap().args().unwrap().request;
                let (id, action, message, _icon, _app, users, default) = request.clone();
                assert_eq!(action, "org.freedesktop.timedate1.set-timezone");
                assert!(message.contains("time zone"));
                assert_eq!(
                    users,
                    [
                        ("root".to_owned(), "root".to_owned()),
                        ("tester".to_owned(), "Test User".to_owned())
                    ]
                );
                // The user running the session comes first in the dialog.
                assert_eq!(default, "tester");
                assert_eq!(shell.pending().await.unwrap(), [request]);
                assert!(!shell.respond(id, "tester", "wrong").await.unwrap());
                // A user polkit did not offer is refused before the helper runs.
                assert!(shell.respond(id, "mallory", "secret").await.is_err());
                assert!(shell.respond(id, "tester", "secret").await.unwrap());
                let signal = closed.next().await.unwrap();
                let args = signal.args().unwrap();
                assert_eq!((args.id, args.outcome.as_str()), (id, "success"));
            };
            let (result, ()) = future::zip(begin, answer).await;
            result.unwrap();
            assert!(shell.pending().await.unwrap().is_empty());

            // Cancelled by the user: polkit gets the Cancelled error.
            let begin = polkit.begin_authentication(
                "org.freedesktop.hostname1.set-hostname",
                "m",
                "",
                HashMap::new(),
                "cookie-2",
                identities(&[own_uid()]),
            );
            let answer = async {
                let (id, ..) = requested.next().await.unwrap().args().unwrap().request;
                shell.cancel(id).await.unwrap();
                let signal = closed.next().await.unwrap();
                assert_eq!(signal.args().unwrap().id, id);
            };
            let (result, ()) = future::zip(begin, answer).await;
            match result {
                Err(zbus::Error::MethodError(name, _, _)) => {
                    assert_eq!(name.as_str(), "org.freedesktop.PolicyKit1.Error.Cancelled")
                }
                other => panic!("{other:?}"),
            }

            // Cancelled by polkit: the dialog closes.
            let begin = polkit.begin_authentication(
                "org.freedesktop.locale1.set-locale",
                "m",
                "",
                HashMap::new(),
                "cookie-3",
                identities(&[own_uid()]),
            );
            let answer = async {
                let (id, ..) = requested.next().await.unwrap().args().unwrap().request;
                polkit.cancel_authentication("cookie-3").await.unwrap();
                let signal = closed.next().await.unwrap();
                let args = signal.args().unwrap();
                assert_eq!((args.id, args.outcome.as_str()), (id, "cancelled"));
            };
            let (result, ()) = future::zip(begin, answer).await;
            assert!(result.is_err());

            // Nobody but polkit can open the dialog.
            let intruder = agent(&s.other, &s.daemon_system).await;
            let result = intruder
                .begin_authentication(
                    "x",
                    "Fake",
                    "",
                    HashMap::new(),
                    "c",
                    identities(&[own_uid()]),
                )
                .await;
            assert!(result.is_err());
            assert!(shell.pending().await.unwrap().is_empty());
        };
        future::or(test, async {
            let result = s.service.run().await;
            panic!("the agent ended: {result:?}");
        })
        .await;
    });
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_socket_helper_takes_the_user_and_cookie_first() {
    let dir = temp_dir("socket");
    let socket = dir.join("agent-helper.socket");
    let listener = UnixListener::bind(&socket).unwrap();
    let seen = Arc::new(Mutex::new(Vec::<String>::new()));
    let seen_by_helper = Arc::clone(&seen);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = || {
                let mut text = String::new();
                reader.read_line(&mut text).unwrap();
                text.trim_end().to_owned()
            };
            let user = line();
            let cookie = line();
            stream
                .write_all(b"PAM_PROMPT_ECHO_OFF Password: \n")
                .unwrap();
            let password = line();
            seen_by_helper
                .lock()
                .unwrap()
                .push(format!("{user} {cookie} {password}"));
            let verdict = if password == "secret" {
                "SUCCESS"
            } else {
                "FAILURE"
            };
            stream.write_all(format!("{verdict}\n").as_bytes()).unwrap();
        }
    });
    let config = AuthConfig {
        system_bus: None,
        helper: dir.join("missing-helper"),
        helper_socket: Some(socket),
        session_id: Some("3".to_owned()),
        passwd: passwd(&dir),
    };
    zbus::block_on(async {
        let s = setup(Bus::start(), Bus::start(), config).await;
        let test = async {
            let shell = Authentication1Proxy::builder(&s.client)
                .destination(s.daemon_session.as_str())
                .unwrap()
                .build()
                .await
                .unwrap();
            let mut requested = shell.receive_requested().await.unwrap();
            let polkit = agent(&s.polkit, &s.daemon_system).await;
            let begin = polkit.begin_authentication(
                "a",
                "m",
                "",
                HashMap::new(),
                "cookie-s",
                identities(&[own_uid()]),
            );
            let answer = async {
                let (id, ..) = requested.next().await.unwrap().args().unwrap().request;
                assert!(!shell.respond(id, "tester", "nope").await.unwrap());
                assert!(shell.respond(id, "tester", "secret").await.unwrap());
            };
            let (result, ()) = future::zip(begin, answer).await;
            result.unwrap();
            assert_eq!(
                *seen.lock().unwrap(),
                ["tester cookie-s nope", "tester cookie-s secret"]
            );
        };
        future::or(test, async {
            let result = s.service.run().await;
            panic!("the agent ended: {result:?}");
        })
        .await;
    });
    let _ = std::fs::remove_dir_all(&dir);
}
