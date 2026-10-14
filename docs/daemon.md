# silica-daemon

Silica's system services on D-Bus (`crates/daemon`, binary `silica-daemon`). Today the
notifications; the Control Center (network, Bluetooth, battery, brightness, audio) arrives in
M4.4.

```
silica-daemon [--address ADDRESS] [--replace]
```

It connects to the session bus (`DBUS_SESSION_BUS_ADDRESS`, or `--address`) and takes the
names `org.freedesktop.Notifications` and `dev.frascella.Silica`. If another notification
server already has them (nested in GNOME), it exits with an error; `--replace` takes them if
the owner allows it.

## Private session bus

`ossido --session-bus` (passed by `cargo xtask run --shell`) starts its own `dbus-daemon` and
`silica-daemon` on that bus, prints `DBUS_SESSION_BUS_ADDRESS=…` on stdout and passes the
address to all the clients it launches (`--spawn`, `--spawn-shell`, `app.launch`).
Nested in GNOME, the names of the host session stay its own; the system bus is the
machine's.

`silica-daemon` is looked for next to Ossido's executable, then in `PATH`; if it is missing,
Ossido warns and goes on without notifications. The bus dies with Ossido even if Ossido is
killed (`dbus-daemon` runs under a shell that terminates when the stdin connected to Ossido
closes) and `silica-daemon` exits when it loses the bus.

## The silica/dbus channel

The embedder gives Dart D-Bus with JSON values (`crates/embedder/src/dbus`, Dart side
`shell/lib/system/dbus.dart`), on a separate thread. Codec `JSONMethodCodec`:

- `call {bus?, destination, path, interface?, method, signature?, args?}` → the list of the
  reply's arguments. `bus` is `session` (default) or `system`. The arguments are converted
  according to `signature` (`"su"`, `"a{sv}"`…); inside a variant the type is deduced from
  the JSON (integers `i` or `x`, decimals `d`, string lists `as`, objects `a{sv}`).
  Structures → lists, dictionaries → maps. A D-Bus error becomes a `PlatformException` with
  the error name as the code; after 25 s `org.freedesktop.DBus.Error.Timeout`.
- `subscribe {bus?, sender?, path?, interface?, member?}` → an id; signals arrive as the
  call `signal {id, sender, path, interface, member, args}` (at most 256
  subscriptions). `unsubscribe {id}` → `true` if it existed.

In Dart: `DBus.instance.call(...)`, `getProperty`, `getAll` and `signals(...)` (a stream:
the subscription closes when it is cancelled).

## org.freedesktop.Notifications

The [freedesktop specification](https://specifications.freedesktop.org/notification-spec/) 1.2
on `/org/freedesktop/Notifications`, for the apps (`notify-send`, libnotify, portals).

- `Notify`: returns the id (never 0). With the `replaces_id` of a notification still present
  it replaces it and keeps the id.
- `CloseNotification`: closes with reason 3; an id that no longer exists is not an error.
- `GetCapabilities`: `actions`, `body`, `icon-static`, `persistence`. No markup: the
  shell shows the body as text.
- `GetServerInformation`: `Silica`, `Silica`, crate version, `1.2`.
- Signals `NotificationClosed(id, reason)` and `ActionInvoked(id, key)`.

Hints read: `urgency` (byte or integer), `category`, `desktop-entry`, `image-path`,
`transient`, `resident` (booleans or integers). The others are ignored, as are wrong types.

Expiry: as in the reference's Notification Center, a notification stays in the history until
the user closes it. It expires (reason 1) only with a positive `expire_timeout`; -1 and 0 do
not expire. The banner time is decided by the shell.

Limits against hostile clients: title 1024 characters, body 8192, names 256, 32 action
entries (a key without a label is discarded), history of 256 notifications (beyond that the
oldest is closed with reason 4).

## dev.frascella.Silica.Notifications1

On `/dev/frascella/Silica/Notifications1`, for the shell (banners and Notification Center).

- `List() → a(ussssasysssbbti)`: the history, oldest first. Fields: id, app, icon,
  title, body, actions (key, label, flattened), urgency, category,
  desktop-entry, image, transient, resident, reception in Unix ms, `expire_timeout`.
- `Dismiss(id)`: closed by the user (reason 2). `Clear()`: all of them.
- `InvokeAction(id, key)`: emits `ActionInvoked`; without `resident` it then closes with
  reason 2. A missing key is the `InvalidArgs` error.
- `Reply(id, text)`: for notifications with the `inline-reply` action (KDE's extension,
  announced in `GetCapabilities`): emits `NotificationReplied(id, text)` on
  `org.freedesktop.Notifications`; without `resident` it then closes with reason 2.
- Signals `Added(notification)` (new or replaced: same id) and `Removed(id, reason)`.

## Trying it by hand

```
dbus-daemon --session --fork --print-address    # prints the address
silica-daemon --address unix:path=…
DBUS_SESSION_BUS_ADDRESS=unix:path=… notify-send Hello "from the daemon"
DBUS_SESSION_BUS_ADDRESS=unix:path=… gdbus call --session -d dev.frascella.Silica \
  -o /dev/frascella/Silica/Notifications1 -m dev.frascella.Silica.Notifications1.List
```

On Ubuntu the AppArmor profile of `notify-send` blocks its `GetCapabilities` call towards an
unconfined server: `notify-send` then shows the notification without actions. The actions
are tried with the tests (`cargo test -p silica-daemon`, on a private `dbus-daemon`).

## dev.frascella.Silica.System1

On `/dev/frascella/Silica/System1`: the system state for the Control Center and the menu
bar.

- `State() → a{sv}`: flat keys. `wifi.available`, `wifi.enabled`, `wifi.ssid`,
  `wifi.strength` (0–100), `wifi.wired`; `bluetooth.available`, `bluetooth.powered`;
  `battery.present`, `battery.percent`, `battery.charging`; `brightness.available`,
  `brightness` (0–1); `audio.available`, `audio.volume` (0–1), `audio.muted`, `audio.name`
  (name of the output, PipeWire's `node.description`); `dnd`.
- `SetWifiEnabled(b)`, `SetBluetoothPowered(b)`, `SetVolume(d)`, `SetMuted(b)`,
  `SetBrightness(d)` (0–1, never to zero), `SetDoNotDisturb(b)`. A missing source is the
  `org.freedesktop.DBus.Error.Failed` error.
- For the Control Center's detail panels, read on request:
  `WifiNetworks() → a(sybbb)` (name, signal 0–100, secured, saved, in use; the one in use
  first, then the saved ones, then by signal; at most 40), `ConnectWifi(s)` (saved networks
  only: a new network asks for a password), `AudioOutputs() → a(usb)` (PipeWire id, name,
  default; from `wpctl status`), `SetAudioOutput(u)`.
- Signal `Changed(a{sv})` with the whole state on every change.

Sources: NetworkManager (`WirelessEnabled`, primary connection, access point strength),
BlueZ (the first adapter), UPower (`DisplayDevice`, only if it is a battery) on the system
bus, read again on each of their signals; the backlight in `/sys/class/backlight`
(written with logind's `SetBrightness`, otherwise in the file) and the audio of the default
output with `wpctl`, read again every second. If a service is missing the source is absent
(`*.available = false`) and it is retried every 10 s.

## Light and dark appearance

- `System1` has the `appearance.dark` key and the `SetDark(b)` method, which saves the choice
  in `$XDG_CONFIG_HOME/silica/appearance.json` (`{"dark": true}`) for the next start.
- `silica-portal` offers `org.freedesktop.impl.portal.Settings` with
  `org.freedesktop.appearance` › `color-scheme` (1 dark, 2 light): it reads it from the daemon
  and follows its `Changed` (without the daemon, from the file), and emits `SettingChanged`.
  GTK, Qt and libadwaita apps follow it from the frontend; the embedder reads it from the
  backend and sends Flutter `flutter/settings` with `platformBrightness`.
- Ossido starts `silica-portal` together with `silica-daemon` on its private bus; the
  commands `system.dark` and `system.dark_toggle` call `SetDark`.
