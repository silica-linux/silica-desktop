//! The notification daemon on a private session bus (`dbus-daemon`).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::HashMap;
use std::future::Future;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use futures_lite::{StreamExt, future};
use silica_daemon::Daemon;
use silica_daemon::notifications::Notification;
use zbus::zvariant::Value;
use zbus::{Connection, proxy};

#[proxy(
    interface = "org.freedesktop.Notifications",
    default_service = "org.freedesktop.Notifications",
    default_path = "/org/freedesktop/Notifications"
)]
trait Notifications {
    #[allow(clippy::too_many_arguments)]
    fn notify(
        &self,
        app_name: &str,
        replaces_id: u32,
        app_icon: &str,
        summary: &str,
        body: &str,
        actions: &[&str],
        hints: HashMap<&str, Value<'_>>,
        expire_timeout: i32,
    ) -> zbus::Result<u32>;
    fn close_notification(&self, id: u32) -> zbus::Result<()>;
    fn get_capabilities(&self) -> zbus::Result<Vec<String>>;
    fn get_server_information(&self) -> zbus::Result<(String, String, String, String)>;
    #[zbus(signal)]
    fn notification_closed(&self, id: u32, reason: u32) -> zbus::Result<()>;
    #[zbus(signal)]
    fn action_invoked(&self, id: u32, action_key: String) -> zbus::Result<()>;
}

#[proxy(
    interface = "dev.frascella.Silica.Notifications1",
    default_service = "dev.frascella.Silica",
    default_path = "/dev/frascella/Silica/Notifications1"
)]
trait Shell {
    fn list(&self) -> zbus::Result<Vec<Notification>>;
    fn dismiss(&self, id: u32) -> zbus::Result<()>;
    fn clear(&self) -> zbus::Result<()>;
    fn invoke_action(&self, id: u32, action_key: &str) -> zbus::Result<()>;
    #[zbus(signal)]
    fn added(&self, notification: Notification) -> zbus::Result<()>;
    #[zbus(signal)]
    fn removed(&self, id: u32, reason: u32) -> zbus::Result<()>;
}

/// A private `dbus-daemon`, closed at the end of the test.
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

/// Starts the daemon on a new bus and runs `test` with a client connection.
fn with_daemon<F, Fut>(test: F)
where
    F: FnOnce(Connection) -> Fut,
    Fut: Future<Output = ()>,
{
    let bus = Bus::start();
    zbus::block_on(async {
        let daemon = Daemon::serve(bus.connect().await, false).await.unwrap();
        let client = bus.connect().await;
        let run = async {
            let result = daemon.run().await;
            panic!("the daemon loop ended: {result:?}");
        };
        let test = async {
            future::or(test(client), async {
                async_io::Timer::after(Duration::from_secs(20)).await;
                panic!("test over the time limit");
            })
            .await;
        };
        future::or(test, run).await;
    });
}

fn no_hints() -> HashMap<&'static str, Value<'static>> {
    HashMap::new()
}

#[test]
fn server_information_and_capabilities() {
    with_daemon(|conn| async move {
        let fdo = NotificationsProxy::new(&conn).await.unwrap();
        let (name, _, _, spec) = fdo.get_server_information().await.unwrap();
        assert_eq!(name, "Silica");
        assert_eq!(spec, "1.2");
        let caps = fdo.get_capabilities().await.unwrap();
        assert!(caps.iter().any(|c| c == "actions"));
        assert!(caps.iter().any(|c| c == "persistence"));
    });
}

#[test]
fn notify_reaches_the_shell() {
    with_daemon(|conn| async move {
        let fdo = NotificationsProxy::new(&conn).await.unwrap();
        let shell = ShellProxy::new(&conn).await.unwrap();
        let mut added = shell.receive_added().await.unwrap();

        let mut hints = no_hints();
        hints.insert("urgency", Value::U8(2));
        hints.insert("desktop-entry", Value::from("org.example.Mail"));
        hints.insert("transient", Value::Bool(true));
        let id = fdo
            .notify("Mail", 0, "mail", "New message", "Hello", &[], hints, -1)
            .await
            .unwrap();
        assert_ne!(id, 0);

        let signal = added.next().await.unwrap();
        let n = signal.args().unwrap().notification;
        assert_eq!(n.id, id);
        assert_eq!(n.app_name, "Mail");
        assert_eq!(n.summary, "New message");
        assert_eq!(n.urgency, 2);
        assert_eq!(n.desktop_entry, "org.example.Mail");
        assert!(n.transient);
        assert!(n.timestamp_ms > 0);
        assert_eq!(shell.list().await.unwrap(), std::slice::from_ref(&n));

        // Replacement keeps the id and updates the history.
        let again = fdo
            .notify("Mail", id, "mail", "Two messages", "", &[], no_hints(), -1)
            .await
            .unwrap();
        assert_eq!(again, id);
        let list = shell.list().await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].summary, "Two messages");
    });
}

#[test]
fn close_and_dismiss_emit_reasons() {
    with_daemon(|conn| async move {
        let fdo = NotificationsProxy::new(&conn).await.unwrap();
        let shell = ShellProxy::new(&conn).await.unwrap();
        let mut closed = fdo.receive_notification_closed().await.unwrap();
        let mut removed = shell.receive_removed().await.unwrap();

        let a = fdo
            .notify("app", 0, "", "a", "", &[], no_hints(), -1)
            .await
            .unwrap();
        let b = fdo
            .notify("app", 0, "", "b", "", &[], no_hints(), -1)
            .await
            .unwrap();

        fdo.close_notification(a).await.unwrap();
        let args = closed.next().await.unwrap();
        assert_eq!(
            (args.args().unwrap().id, args.args().unwrap().reason),
            (a, 3)
        );
        let args = removed.next().await.unwrap();
        assert_eq!(
            (args.args().unwrap().id, args.args().unwrap().reason),
            (a, 3)
        );

        shell.dismiss(b).await.unwrap();
        let args = closed.next().await.unwrap();
        assert_eq!(
            (args.args().unwrap().id, args.args().unwrap().reason),
            (b, 2)
        );
        assert!(shell.list().await.unwrap().is_empty());

        // Closing a notification that is gone is not an error.
        fdo.close_notification(b).await.unwrap();
        shell.dismiss(12345).await.unwrap();
    });
}

#[test]
fn actions_are_forwarded_and_close() {
    with_daemon(|conn| async move {
        let fdo = NotificationsProxy::new(&conn).await.unwrap();
        let shell = ShellProxy::new(&conn).await.unwrap();
        let mut invoked = fdo.receive_action_invoked().await.unwrap();
        let mut closed = fdo.receive_notification_closed().await.unwrap();

        let actions = ["default", "Open", "reply", "Reply"];
        let id = fdo
            .notify("app", 0, "", "actions", "", &actions, no_hints(), -1)
            .await
            .unwrap();
        assert!(shell.invoke_action(id, "missing").await.is_err());
        shell.invoke_action(id, "reply").await.unwrap();
        let args = invoked.next().await.unwrap();
        assert_eq!(args.args().unwrap().id, id);
        assert_eq!(args.args().unwrap().action_key, "reply");
        let args = closed.next().await.unwrap();
        assert_eq!(
            (args.args().unwrap().id, args.args().unwrap().reason),
            (id, 2)
        );

        // With `resident` the notification stays.
        let mut hints = no_hints();
        hints.insert("resident", Value::Bool(true));
        let id = fdo
            .notify("app", 0, "", "stays", "", &actions, hints, -1)
            .await
            .unwrap();
        shell.invoke_action(id, "default").await.unwrap();
        invoked.next().await.unwrap();
        assert_eq!(shell.list().await.unwrap().len(), 1);
        shell.clear().await.unwrap();
        assert!(shell.list().await.unwrap().is_empty());
    });
}

#[test]
fn explicit_timeouts_expire() {
    with_daemon(|conn| async move {
        let fdo = NotificationsProxy::new(&conn).await.unwrap();
        let shell = ShellProxy::new(&conn).await.unwrap();
        let mut closed = fdo.receive_notification_closed().await.unwrap();

        let stays = fdo
            .notify("app", 0, "", "stays", "", &[], no_hints(), -1)
            .await
            .unwrap();
        let first = fdo
            .notify("app", 0, "", "200 ms", "", &[], no_hints(), 200)
            .await
            .unwrap();
        let args = closed.next().await.unwrap();
        assert_eq!(
            (args.args().unwrap().id, args.args().unwrap().reason),
            (first, 1)
        );
        let ids: Vec<u32> = shell.list().await.unwrap().iter().map(|n| n.id).collect();
        assert_eq!(ids, [stays]);
    });
}

#[test]
fn hostile_hints_do_not_break_the_server() {
    with_daemon(|conn| async move {
        let fdo = NotificationsProxy::new(&conn).await.unwrap();
        let shell = ShellProxy::new(&conn).await.unwrap();
        let mut hints = no_hints();
        // Wrong types: ignored.
        hints.insert("urgency", Value::from("alta"));
        hints.insert("transient", Value::from(7u32));
        hints.insert("desktop-entry", Value::U8(3));
        let body = "x".repeat(1 << 20);
        let id = fdo
            .notify(
                "app",
                0,
                "",
                "hostile",
                &body,
                &["key-only"],
                hints,
                i32::MIN,
            )
            .await
            .unwrap();
        let n = &shell.list().await.unwrap()[0];
        assert_eq!(n.id, id);
        assert_eq!(n.urgency, 1);
        assert!(n.transient);
        assert!(n.desktop_entry.is_empty());
        assert!(n.actions.is_empty());
        assert!(n.body.len() < body.len());
    });
}

#[test]
fn a_second_server_cannot_take_the_names() {
    let bus = Bus::start();
    zbus::block_on(async {
        let _first = Daemon::serve(bus.connect().await, false).await.unwrap();
        let second = Daemon::serve(bus.connect().await, false).await;
        assert!(matches!(second, Err(zbus::Error::NameTaken)));
        // With --replace it takes the place of the first one, which allows it.
        assert!(Daemon::serve(bus.connect().await, true).await.is_ok());
    });
}
